pub mod memory;
pub mod sled_store;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Metadata about a stored flagfile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub hash: String,
    pub pushed_at: String,
    pub flags_count: u64,
}

/// Storage trait for flagfile content. Implementations must be thread-safe.
#[async_trait]
pub trait FlagStore: Send + Sync {
    /// Get raw flagfile content for a namespace. Root namespace uses `ROOT_NAMESPACE`.
    async fn get_flagfile(&self, namespace: &str) -> Option<Vec<u8>>;

    /// Store flagfile content with metadata.
    async fn put_flagfile(
        &self,
        namespace: &str,
        content: &[u8],
        meta: &Meta,
    ) -> Result<(), String>;

    /// Get metadata for a namespace.
    async fn get_meta(&self, namespace: &str) -> Option<Meta>;

    /// List all namespaces that have stored flagfiles.
    async fn list_namespaces(&self) -> Vec<String>;

    /// Apply a snapshot (for Raft recovery).
    async fn apply_snapshot(&self, data: &[u8]) -> Result<(), String>;

    /// Create a snapshot of all data (for Raft).
    async fn create_snapshot(&self) -> Result<Vec<u8>, String>;
}

/// The key used for the root (global) namespace.
pub const ROOT_NAMESPACE: &str = "__root__";
