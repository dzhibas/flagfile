use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{FlagStore, Meta};

/// Persistent flagfile storage backed by sled.
pub struct SledStore {
    db: sled::Db,
}

/// Serializable snapshot of the entire store.
#[derive(Serialize, Deserialize)]
struct Snapshot {
    entries: HashMap<String, SnapshotEntry>,
}

#[derive(Serialize, Deserialize)]
struct SnapshotEntry {
    content: Vec<u8>,
    meta: Meta,
}

impl SledStore {
    pub fn new(db: sled::Db) -> Self {
        Self { db }
    }

    /// Open a sled database at the given directory path.
    pub fn open(data_dir: &str) -> Result<Self, String> {
        let db = sled::open(data_dir).map_err(|e| format!("failed to open sled db: {}", e))?;
        Ok(Self::new(db))
    }

    fn flags_key(namespace: &str) -> String {
        format!("flags:{}", namespace)
    }

    fn meta_key(namespace: &str) -> String {
        format!("meta:{}", namespace)
    }
}

#[async_trait]
impl FlagStore for SledStore {
    async fn get_flagfile(&self, namespace: &str) -> Option<Vec<u8>> {
        self.db
            .get(Self::flags_key(namespace))
            .ok()?
            .map(|ivec| ivec.to_vec())
    }

    async fn put_flagfile(
        &self,
        namespace: &str,
        content: &[u8],
        meta: &Meta,
    ) -> Result<(), String> {
        let meta_bytes = serde_json::to_vec(meta)
            .map_err(|e| format!("failed to serialize meta: {}", e))?;

        self.db
            .insert(Self::flags_key(namespace), content)
            .map_err(|e| format!("failed to store flagfile: {}", e))?;

        self.db
            .insert(Self::meta_key(namespace), meta_bytes)
            .map_err(|e| format!("failed to store meta: {}", e))?;

        self.db
            .flush()
            .map_err(|e| format!("failed to flush: {}", e))?;

        Ok(())
    }

    async fn get_meta(&self, namespace: &str) -> Option<Meta> {
        let ivec = self.db.get(Self::meta_key(namespace)).ok()??;
        serde_json::from_slice(&ivec).ok()
    }

    async fn list_namespaces(&self) -> Vec<String> {
        let prefix = "flags:";
        self.db
            .scan_prefix(prefix)
            .filter_map(|item| {
                let (key, _) = item.ok()?;
                let key_str = std::str::from_utf8(&key).ok()?;
                Some(key_str.strip_prefix(prefix)?.to_string())
            })
            .collect()
    }

    async fn apply_snapshot(&self, data: &[u8]) -> Result<(), String> {
        let snapshot: Snapshot = serde_json::from_slice(data)
            .map_err(|e| format!("failed to deserialize snapshot: {}", e))?;

        self.db
            .clear()
            .map_err(|e| format!("failed to clear db: {}", e))?;

        for (namespace, entry) in &snapshot.entries {
            let meta_bytes = serde_json::to_vec(&entry.meta)
                .map_err(|e| format!("failed to serialize meta: {}", e))?;

            self.db
                .insert(Self::flags_key(namespace), entry.content.as_slice())
                .map_err(|e| format!("failed to insert flagfile: {}", e))?;

            self.db
                .insert(Self::meta_key(namespace), meta_bytes)
                .map_err(|e| format!("failed to insert meta: {}", e))?;
        }

        self.db
            .flush()
            .map_err(|e| format!("failed to flush: {}", e))?;

        Ok(())
    }

    async fn create_snapshot(&self) -> Result<Vec<u8>, String> {
        let prefix = "flags:";
        let mut entries = HashMap::new();

        for item in self.db.scan_prefix(prefix) {
            let (key, value) = item.map_err(|e| format!("failed to read key: {}", e))?;
            let key_str = std::str::from_utf8(&key)
                .map_err(|e| format!("invalid key encoding: {}", e))?;
            let namespace = key_str
                .strip_prefix(prefix)
                .ok_or_else(|| "unexpected key format".to_string())?;

            let meta = self.get_meta(namespace).await.ok_or_else(|| {
                format!("meta missing for namespace: {}", namespace)
            })?;

            entries.insert(
                namespace.to_string(),
                SnapshotEntry {
                    content: value.to_vec(),
                    meta,
                },
            );
        }

        let snapshot = Snapshot { entries };
        serde_json::to_vec(&snapshot)
            .map_err(|e| format!("failed to serialize snapshot: {}", e))
    }
}
