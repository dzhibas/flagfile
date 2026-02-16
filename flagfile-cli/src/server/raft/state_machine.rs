use std::sync::Arc;

use super::RaftCommand;
use crate::server::sse::FlagUpdateEvent;
use crate::server::state::{AppState, ParsedNamespace};
use crate::server::store::FlagStore;
use crate::server::watch::parse_flags;

/// Applies committed Raft entries to the underlying flag store,
/// updates in-memory parsed state, and broadcasts SSE events.
pub struct RaftStateMachine {
    store: Arc<dyn FlagStore + Send + Sync>,
    state: Arc<AppState>,
}

impl RaftStateMachine {
    pub fn new(store: Arc<dyn FlagStore + Send + Sync>, state: Arc<AppState>) -> Self {
        Self { store, state }
    }

    /// Apply a committed Raft command to the store and update in-memory state.
    pub async fn apply(&self, cmd: RaftCommand) {
        match cmd {
            RaftCommand::PutFlagfile {
                namespace,
                content,
                meta,
            } => {
                let hash = meta.hash.clone();
                let flags_count = meta.flags_count;

                if let Err(e) = self.store.put_flagfile(&namespace, &content, &meta).await {
                    eprintln!(
                        "raft: failed to apply PutFlagfile for {}: {}",
                        namespace, e
                    );
                    return;
                }

                // Parse and update in-memory namespace state.
                if let Ok(content_str) = String::from_utf8(content) {
                    if let Some((flags, metadata, segments)) = parse_flags(&content_str) {
                        let env = {
                            let ns = self.state.namespaces.read().await;
                            ns.get(&namespace).and_then(|n| n.env.clone())
                        };

                        let mut ns_map = self.state.namespaces.write().await;
                        ns_map.insert(
                            namespace.clone(),
                            ParsedNamespace {
                                flagfile_content: content_str,
                                flags,
                                metadata,
                                segments,
                                env,
                            },
                        );
                    }
                }

                // Broadcast SSE update.
                self.state
                    .broadcaster
                    .broadcast(
                        &namespace,
                        FlagUpdateEvent {
                            hash,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            flags_count,
                        },
                    )
                    .await;
            }
        }
    }

    /// Create a snapshot of all current store state (serialised as bytes).
    pub async fn snapshot(&self) -> Result<Vec<u8>, String> {
        self.store.create_snapshot().await
    }

    /// Restore the store from a previously created snapshot and reload
    /// in-memory parsed namespaces.
    pub async fn restore(&self, data: &[u8]) -> Result<(), String> {
        self.store.apply_snapshot(data).await?;

        // Reload all namespaces from store into memory.
        let stored_ns = self.store.list_namespaces().await;
        let mut ns_map = self.state.namespaces.write().await;
        ns_map.clear();

        for ns_key in stored_ns {
            if let Some(content_bytes) = self.store.get_flagfile(&ns_key).await {
                if let Ok(content) = String::from_utf8(content_bytes) {
                    if let Some((flags, metadata, segments)) = parse_flags(&content) {
                        ns_map.insert(
                            ns_key,
                            ParsedNamespace {
                                flagfile_content: content,
                                flags,
                                metadata,
                                segments,
                                env: None,
                            },
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
