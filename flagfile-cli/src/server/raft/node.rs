use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use raft::prelude::*;
use raft::RawNode;
use slog::o;
use tokio::sync::{mpsc, oneshot};
use tokio::time;

use super::state_machine::RaftStateMachine;
use super::storage::MemRaftStorage;
use super::transport::RaftTransport;
use super::RaftCommand;
use crate::server::config::ClusterConfig;
use crate::server::metrics::metrics;

/// Proposal sent to the Raft node for replication.
pub struct Proposal {
    pub data: Vec<u8>,
    pub response_tx: oneshot::Sender<Result<(), String>>,
}

/// Commands sent to the Raft node loop (besides proposals and messages).
pub enum RaftNodeCommand {
    TransferLeader {
        response_tx: oneshot::Sender<Result<(), String>>,
    },
}

/// Handle for interacting with the Raft node from HTTP handlers.
#[derive(Clone)]
pub struct RaftHandle {
    proposal_tx: mpsc::Sender<Proposal>,
    command_tx: mpsc::Sender<RaftNodeCommand>,
    leader_id: Arc<AtomicU64>,
    node_id: u64,
    peer_ids: Vec<u64>,
}

impl RaftHandle {
    /// Propose a command to the Raft cluster. Returns when the proposal has
    /// been submitted (not necessarily committed).
    pub async fn propose(&self, cmd: RaftCommand) -> Result<(), String> {
        let data = serde_json::to_vec(&cmd).map_err(|e| e.to_string())?;
        let (tx, rx) = oneshot::channel();
        let proposal = Proposal {
            data,
            response_tx: tx,
        };

        self.proposal_tx
            .send(proposal)
            .await
            .map_err(|_| "raft node shut down".to_string())?;

        rx.await.map_err(|_| "proposal dropped".to_string())?
    }

    /// Check whether this node believes itself to be the leader.
    pub fn is_leader(&self) -> bool {
        self.leader_id.load(Ordering::Relaxed) == self.node_id
    }

    /// Get the current leader's node ID (0 if unknown).
    pub fn leader_id(&self) -> u64 {
        self.leader_id.load(Ordering::Relaxed)
    }

    /// Get this node's ID.
    pub fn node_id(&self) -> u64 {
        self.node_id
    }

    /// Transfer leadership to another node in the cluster. Returns once the
    /// transfer has been initiated (the caller should poll `is_leader()` to
    /// confirm the transfer completed).
    pub async fn transfer_leader(&self) -> Result<(), String> {
        if self.peer_ids.is_empty() {
            return Err("no peers to transfer leadership to".to_string());
        }
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(RaftNodeCommand::TransferLeader { response_tx: tx })
            .await
            .map_err(|_| "raft node shut down".to_string())?;
        rx.await.map_err(|_| "command dropped".to_string())?
    }
}

/// Channel through which external gRPC messages are stepped into the Raft node.
pub type RaftMsgSender = mpsc::Sender<Message>;

/// Start the Raft node and return a handle plus a channel for incoming Raft
/// messages (from the gRPC transport layer).
///
/// The node runs in a background tokio task until the returned handle is
/// dropped (all senders closed).
pub async fn run_raft_node(
    cluster_cfg: &ClusterConfig,
    storage: MemRaftStorage,
    transport: Arc<RaftTransport>,
    state_machine: Arc<RaftStateMachine>,
) -> (RaftHandle, RaftMsgSender) {
    let node_id = cluster_cfg.node_id;

    // Discard raft-internal logs — errors are surfaced via eprintln.
    let logger = slog::Logger::root(slog::Discard, o!("node" => node_id));

    // Raft configuration.
    let cfg = Config {
        id: node_id,
        election_tick: (cluster_cfg.election_timeout_ms / 100).max(1) as usize,
        heartbeat_tick: (cluster_cfg.heartbeat_interval_ms / 100).max(1) as usize,
        ..Default::default()
    };
    cfg.validate().expect("invalid raft config");

    let mut raw_node =
        RawNode::new(&cfg, storage.clone(), &logger).expect("failed to create raft node");

    // Single-node: campaign immediately so we self-elect.
    let is_single_node = cluster_cfg.peers.is_empty();
    if is_single_node {
        raw_node.raft.become_candidate();
        raw_node.raft.become_leader();
    }

    let (proposal_tx, mut proposal_rx) = mpsc::channel::<Proposal>(256);
    let (raft_msg_tx, mut raft_msg_rx) = mpsc::channel::<Message>(256);
    let (command_tx, mut command_rx) = mpsc::channel::<RaftNodeCommand>(16);

    let leader_id = Arc::new(AtomicU64::new(0));
    let leader_id_clone = Arc::clone(&leader_id);

    let peer_ids: Vec<u64> = cluster_cfg.peers.iter().map(|p| p.id).collect();

    let handle = RaftHandle {
        proposal_tx,
        command_tx,
        leader_id: Arc::clone(&leader_id),
        node_id,
        peer_ids: peer_ids.clone(),
    };

    let tick_ms = 100;
    let snapshot_threshold = cluster_cfg.snapshot_threshold;

    // For multi-node clusters, stagger the initial campaign to avoid split votes.
    // The lowest node_id campaigns first (after ~1s), others wait longer.
    let campaign_at_tick: usize = if is_single_node {
        0
    } else {
        10 + (node_id as usize % 10) * 5
    };

    tokio::spawn(async move {
        let mut tick_interval = time::interval(Duration::from_millis(tick_ms));
        let mut pending: Vec<oneshot::Sender<Result<(), String>>> = Vec::new();
        let mut applied_index: u64 = 0;
        let mut tick_count: usize = 0;
        let mut last_leader: u64 = 0;
        let node_id_str = node_id.to_string();

        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    raw_node.tick();
                    tick_count += 1;

                    // Trigger initial election after a short staggered delay.
                    if !is_single_node && tick_count == campaign_at_tick {
                        println!("raft node {}: triggering initial election", node_id);
                        if let Err(e) = raw_node.campaign() {
                            eprintln!("raft node {}: campaign error: {}", node_id, e);
                        }
                    }
                }
                Some(proposal) = proposal_rx.recv() => {
                    if let Err(e) = raw_node.propose(vec![], proposal.data) {
                        let _ = proposal.response_tx.send(Err(e.to_string()));
                    } else {
                        pending.push(proposal.response_tx);
                    }
                }
                Some(msg) = raft_msg_rx.recv() => {
                    if let Err(e) = raw_node.step(msg) {
                        eprintln!("raft step error: {}", e);
                    }
                }
                Some(cmd) = command_rx.recv() => {
                    match cmd {
                        RaftNodeCommand::TransferLeader { response_tx } => {
                            // Pick the first peer as the transfer target.
                            if let Some(&target) = peer_ids.first() {
                                raw_node.transfer_leader(target);
                                let _ = response_tx.send(Ok(()));
                            } else {
                                let _ = response_tx.send(Err("no peers".to_string()));
                            }
                        }
                    }
                }
                else => break,
            }

            // Update leader tracking and log changes.
            let current_leader = raw_node.raft.leader_id;
            if current_leader != last_leader {
                if current_leader == 0 {
                    println!("raft node {}: leader unknown", node_id);
                } else if current_leader == node_id {
                    println!("raft node {}: became leader (term {})", node_id, raw_node.raft.term);
                    metrics().raft_elections.with_label_values(&[&node_id_str]).inc();
                } else {
                    println!(
                        "raft node {}: following leader {} (term {})",
                        node_id, current_leader, raw_node.raft.term
                    );
                }
                let m = metrics();
                let state_val = if current_leader == 0 { 0 } else if current_leader == node_id { 3 } else { 1 };
                m.raft_state.with_label_values(&[&node_id_str]).set(state_val);
                m.raft_leader_id.with_label_values(&[&node_id_str]).set(current_leader as i64);
                m.raft_term.with_label_values(&[&node_id_str]).set(raw_node.raft.term as i64);
                last_leader = current_leader;
            }
            leader_id_clone.store(current_leader, Ordering::Relaxed);

            // Process ready states.
            if !raw_node.has_ready() {
                continue;
            }

            let mut ready = raw_node.ready();
            // 1. Persist hard state and entries.
            if let Some(hs) = ready.hs() {
                storage.set_hard_state(hs.clone());
                metrics().raft_committed.with_label_values(&[&node_id_str]).set(hs.commit as i64);
            }
            if !ready.entries().is_empty() {
                if let Err(e) = storage.append(ready.entries()) {
                    eprintln!("raft storage append error: {}", e);
                }
            }

            // 2. Apply snapshot if present.
            if !ready.snapshot().is_empty() {
                let snap = ready.snapshot().clone();
                if let Err(e) = state_machine.restore(&snap.data).await {
                    eprintln!("raft snapshot restore error: {}", e);
                }
                if let Err(e) = storage.apply_snapshot(snap) {
                    eprintln!("raft storage apply_snapshot error: {}", e);
                }
            }

            // 3. Send immediate messages to peers.
            for msg in ready.take_messages() {
                let transport = Arc::clone(&transport);
                let to = msg.to;
                tokio::spawn(async move {
                    if let Err(e) = transport.send_raft_message(to, &msg).await {
                        eprintln!("raft transport send to {} error: {}", to, e);
                    }
                });
            }

            // 3b. Send persisted messages (e.g. vote requests that must
            //     be sent after hard state is persisted).
            for msg in ready.take_persisted_messages() {
                let transport = Arc::clone(&transport);
                let to = msg.to;
                tokio::spawn(async move {
                    if let Err(e) = transport.send_raft_message(to, &msg).await {
                        eprintln!("raft transport send to {} error: {}", to, e);
                    }
                });
            }

            // 4. Apply committed entries.
            let committed = ready.take_committed_entries();
            for entry in &committed {
                if entry.data.is_empty() {
                    // Configuration change or empty entry.
                    continue;
                }

                match serde_json::from_slice::<RaftCommand>(&entry.data) {
                    Ok(cmd) => {
                        state_machine.apply(cmd).await;
                    }
                    Err(e) => {
                        eprintln!("raft: failed to deserialise committed entry: {}", e);
                    }
                }

                applied_index = entry.index;
            }

            // Notify pending proposals that their entries have been committed.
            // This is a simplified approach: we drain all pending senders once
            // any committed entries come through.
            if !committed.is_empty() {
                metrics().raft_last_applied.with_label_values(&[&node_id_str]).set(applied_index as i64);
                for tx in pending.drain(..) {
                    let _ = tx.send(Ok(()));
                }
            }

            // 5. Advance the Raft node.
            let mut light_rd = raw_node.advance(ready);

            // Persist any additional entries from the light ready.
            if let Some(commit) = light_rd.commit_index() {
                let mut hs = storage
                    .initial_state()
                    .expect("storage initial_state")
                    .hard_state;
                hs.commit = commit;
                storage.set_hard_state(hs);
            }
            // Send any additional messages.
            for msg in light_rd.take_messages() {
                let transport = Arc::clone(&transport);
                let to = msg.to;
                tokio::spawn(async move {
                    if let Err(e) = transport.send_raft_message(to, &msg).await {
                        eprintln!("raft transport send to {} error: {}", to, e);
                    }
                });
            }

            // Apply committed entries from light ready.
            for entry in light_rd.take_committed_entries() {
                if entry.data.is_empty() {
                    continue;
                }
                if let Ok(cmd) = serde_json::from_slice::<RaftCommand>(&entry.data) {
                    state_machine.apply(cmd).await;
                    applied_index = entry.index;
                }
            }

            raw_node.advance_apply();

            // 6. Trigger snapshot if enough entries have been applied.
            if snapshot_threshold > 0 && applied_index > 0 && applied_index % snapshot_threshold == 0
            {
                match state_machine.snapshot().await {
                    Ok(data) => {
                        let cs = storage
                            .initial_state()
                            .expect("storage initial_state")
                            .conf_state;
                        if let Err(e) = storage.create_snapshot(applied_index, cs, data) {
                            eprintln!("raft create_snapshot error: {}", e);
                        }
                        if let Err(e) = storage.compact(applied_index) {
                            eprintln!("raft compact error: {}", e);
                        }
                        metrics().raft_snapshots.with_label_values(&[&node_id_str]).inc();
                    }
                    Err(e) => {
                        eprintln!("raft state_machine snapshot error: {}", e);
                    }
                }
            }
        }
    });

    (handle, raft_msg_tx)
}
