use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::{FlagStore, Meta};

/// In-memory flagfile storage backed by a `RwLock<HashMap>`.
pub struct MemoryStore {
    data: RwLock<HashMap<String, (Vec<u8>, Meta)>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl FlagStore for MemoryStore {
    async fn get_flagfile(&self, namespace: &str) -> Option<Vec<u8>> {
        let data = self.data.read().await;
        data.get(namespace).map(|(content, _)| content.clone())
    }

    async fn put_flagfile(
        &self,
        namespace: &str,
        content: &[u8],
        meta: &Meta,
    ) -> Result<(), String> {
        let mut data = self.data.write().await;
        data.insert(namespace.to_string(), (content.to_vec(), meta.clone()));
        Ok(())
    }

    async fn get_meta(&self, namespace: &str) -> Option<Meta> {
        let data = self.data.read().await;
        data.get(namespace).map(|(_, meta)| meta.clone())
    }

    async fn list_namespaces(&self) -> Vec<String> {
        let data = self.data.read().await;
        data.keys().cloned().collect()
    }

    async fn apply_snapshot(&self, snapshot: &[u8]) -> Result<(), String> {
        let deserialized: HashMap<String, (Vec<u8>, Meta)> =
            serde_json::from_slice(snapshot).map_err(|e| format!("failed to deserialize snapshot: {}", e))?;
        let mut data = self.data.write().await;
        *data = deserialized;
        Ok(())
    }

    async fn create_snapshot(&self) -> Result<Vec<u8>, String> {
        let data = self.data.read().await;
        serde_json::to_vec(&*data).map_err(|e| format!("failed to serialize snapshot: {}", e))
    }
}
