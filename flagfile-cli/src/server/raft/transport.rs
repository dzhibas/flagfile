use std::collections::HashMap;
use std::time::Instant;

use protobuf::Message as ProtoMessage;
use raft::prelude::Message;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};

use super::node::{RaftHandle, RaftMsgSender};
use super::RaftCommand;
use crate::server::config::PeerConfig;
use crate::server::metrics::metrics;

pub mod proto {
    tonic::include_proto!("flagfile.raft");
}

use proto::raft_service_client::RaftServiceClient;
use proto::raft_service_server::RaftService;
use proto::{
    RaftMessage, RaftResponse, SnapshotChunk, WriteRequest, WriteResponse,
};

// ── Client-side transport ────────────────────────────

/// Manages gRPC client connections to peer Raft nodes.
pub struct RaftTransport {
    peers: HashMap<u64, String>,
    clients: Mutex<HashMap<u64, RaftServiceClient<Channel>>>,
}

impl RaftTransport {
    /// Create a new transport with the given peer addresses.
    /// Connections are established lazily on first use.
    pub fn new(peers: Vec<PeerConfig>) -> Self {
        let peer_map: HashMap<u64, String> =
            peers.into_iter().map(|p| (p.id, p.addr)).collect();
        Self {
            peers: peer_map,
            clients: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a gRPC client for the given peer.
    async fn get_client(
        &self,
        target: u64,
    ) -> Result<RaftServiceClient<Channel>, String> {
        let mut clients = self.clients.lock().await;

        if let Some(client) = clients.get(&target) {
            return Ok(client.clone());
        }

        let addr = self
            .peers
            .get(&target)
            .ok_or_else(|| format!("unknown peer: {}", target))?;

        let endpoint = format!("http://{}", addr);
        let client = RaftServiceClient::connect(endpoint)
            .await
            .map_err(|e| format!("connect to peer {}: {}", target, e))?;

        clients.insert(target, client.clone());
        Ok(client)
    }

    /// Serialise and send a Raft message to the target peer via gRPC.
    pub async fn send_raft_message(
        &self,
        target: u64,
        msg: &Message,
    ) -> Result<(), String> {
        let start = Instant::now();
        let peer = target.to_string();
        let data = msg.write_to_bytes().map_err(|e| format!("encode: {}", e))?;
        let mut client = self.get_client(target).await?;

        let request = Request::new(RaftMessage { data });
        let result = client
            .send_message(request)
            .await
            .map_err(|e| format!("send to {}: {}", target, e));

        let m = metrics();
        m.grpc_requests.with_label_values(&[&peer, "send_message"]).inc();
        m.grpc_latency.with_label_values(&[&peer]).observe(start.elapsed().as_secs_f64());
        if result.is_err() {
            m.grpc_errors.with_label_values(&[&peer]).inc();
        }

        result?;
        Ok(())
    }

    /// Forward a write request to the current leader node.
    pub async fn forward_write(
        &self,
        leader_id: u64,
        namespace: &str,
        content: &[u8],
        token: &str,
    ) -> Result<WriteResponse, String> {
        let start = Instant::now();
        let peer = leader_id.to_string();
        let mut client = self.get_client(leader_id).await?;

        let request = Request::new(WriteRequest {
            namespace: namespace.to_string(),
            content: content.to_vec(),
            token: token.to_string(),
        });

        let result = client
            .forward_write(request)
            .await
            .map_err(|e| format!("forward write to leader {}: {}", leader_id, e));

        let m = metrics();
        m.grpc_requests.with_label_values(&[&peer, "forward_write"]).inc();
        m.grpc_latency.with_label_values(&[&peer]).observe(start.elapsed().as_secs_f64());
        if result.is_err() {
            m.grpc_errors.with_label_values(&[&peer]).inc();
        }

        Ok(result?.into_inner())
    }

    /// Remove a cached client connection (e.g. after a connection error).
    #[allow(dead_code)]
    pub async fn invalidate_client(&self, target: u64) {
        let mut clients = self.clients.lock().await;
        clients.remove(&target);
    }
}

// ── Server-side gRPC service ─────────────────────────

/// gRPC server implementation for handling incoming Raft messages from peers.
pub struct RaftGrpcService {
    raft_msg_tx: RaftMsgSender,
    raft_handle: RaftHandle,
}

impl RaftGrpcService {
    pub fn new(raft_msg_tx: RaftMsgSender, raft_handle: RaftHandle) -> Self {
        Self {
            raft_msg_tx,
            raft_handle,
        }
    }
}

#[tonic::async_trait]
impl RaftService for RaftGrpcService {
    /// Receive a Raft protocol message from a peer and step it into the
    /// local Raft node.
    async fn send_message(
        &self,
        request: Request<RaftMessage>,
    ) -> Result<Response<RaftResponse>, Status> {
        metrics().grpc_requests.with_label_values(&["incoming", "send_message"]).inc();

        let data = request.into_inner().data;
        let msg = Message::parse_from_bytes(&data)
            .map_err(|e| {
                metrics().grpc_errors.with_label_values(&["incoming"]).inc();
                Status::invalid_argument(format!("decode raft message: {}", e))
            })?;

        self.raft_msg_tx
            .send(msg)
            .await
            .map_err(|_| {
                metrics().grpc_errors.with_label_values(&["incoming"]).inc();
                Status::internal("raft node shut down")
            })?;

        Ok(Response::new(RaftResponse { success: true }))
    }

    /// Receive a snapshot from the leader.
    async fn send_snapshot(
        &self,
        request: Request<SnapshotChunk>,
    ) -> Result<Response<RaftResponse>, Status> {
        let chunk = request.into_inner();

        // For now, snapshots arrive as a single chunk (done == true).
        // Streaming chunked snapshots can be added later.
        if chunk.done {
            // The snapshot data will be applied when the Raft node processes
            // the corresponding MsgSnap — this endpoint serves as the data
            // transport channel for large snapshots that don't fit in a
            // single Raft message.
            // TODO: wire large snapshot transport
        }

        Ok(Response::new(RaftResponse { success: true }))
    }

    /// Handle a forwarded write request from a follower. If this node is the
    /// leader, propose the write to Raft.
    async fn forward_write(
        &self,
        request: Request<WriteRequest>,
    ) -> Result<Response<WriteResponse>, Status> {
        metrics().grpc_requests.with_label_values(&["incoming", "forward_write"]).inc();
        let req = request.into_inner();

        if !self.raft_handle.is_leader() {
            return Ok(Response::new(WriteResponse {
                success: false,
                hash: String::new(),
                flags_count: 0,
                error: "not the leader".to_string(),
            }));
        }

        // Parse the flagfile to compute metadata.
        let content_str = String::from_utf8_lossy(&req.content);
        let flags_count = crate::server::watch::parse_flags(&content_str)
            .map(|(flags, _, _)| flags.len() as u64)
            .unwrap_or(0);

        let hash = {
            use sha1::Digest;
            let mut hasher = sha1::Sha1::new();
            hasher.update(&req.content);
            format!("{:x}", hasher.finalize())
        };

        let meta = crate::server::store::Meta {
            hash: hash.clone(),
            pushed_at: chrono::Utc::now().to_rfc3339(),
            flags_count,
        };

        let cmd = RaftCommand::PutFlagfile {
            namespace: req.namespace,
            content: req.content,
            meta,
        };

        match self.raft_handle.propose(cmd).await {
            Ok(()) => Ok(Response::new(WriteResponse {
                success: true,
                hash,
                flags_count,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(WriteResponse {
                success: false,
                hash: String::new(),
                flags_count: 0,
                error: e,
            })),
        }
    }
}
