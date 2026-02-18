use std::collections::HashMap;
use std::env;

use serde::Deserialize;

/// Top-level ff-server.toml configuration
#[derive(Debug, Deserialize, Default)]
pub struct FfServerConfig {
    #[serde(default)]
    pub server: ServerConfig,
    pub cluster: Option<ClusterConfig>,
    #[serde(default)]
    pub root: NamespaceConfig,
    #[serde(default)]
    pub namespaces: HashMap<String, NamespaceConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_hostname")]
    pub hostname: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    #[serde(default = "default_storage")]
    pub storage: StorageBackend,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    Sled,
    Memory,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ClusterConfig {
    pub node_id: u64,
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,
    #[serde(default)]
    pub peers: Vec<PeerConfig>,
    #[serde(default = "default_election_timeout")]
    pub election_timeout_ms: u64,
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_ms: u64,
    #[serde(default = "default_snapshot_threshold")]
    pub snapshot_threshold: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PeerConfig {
    pub id: u64,
    pub addr: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct NamespaceConfig {
    #[serde(default)]
    pub read_tokens: Vec<String>,
    #[serde(default)]
    pub write_tokens: Vec<String>,
}

// ── Sidecar config ──────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct SidecarConfig {
    pub upstream: Option<String>,
    pub token: Option<String>,
    pub namespace: Option<String>,
}

// ── Default value functions ──────────────────────────

fn default_port() -> u16 {
    8080
}

fn default_hostname() -> String {
    "0.0.0.0".to_string()
}

fn default_data_dir() -> String {
    "./data".to_string()
}

fn default_storage() -> StorageBackend {
    StorageBackend::Sled
}

fn default_grpc_port() -> u16 {
    9090
}

fn default_election_timeout() -> u64 {
    1000
}

fn default_heartbeat_interval() -> u64 {
    300
}

fn default_snapshot_threshold() -> u64 {
    1000
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            hostname: default_hostname(),
            data_dir: default_data_dir(),
            storage: default_storage(),
        }
    }
}

impl FfServerConfig {
    /// Load configuration from a TOML file, falling back to defaults if the file
    /// doesn't exist or cannot be parsed.
    pub fn load(path: &str) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Warning: failed to parse {}: {}", path, e);
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Apply environment variable overrides to the configuration.
    pub fn apply_env_overrides(&mut self) {
        // FF_STORAGE
        if let Ok(val) = env::var("FF_STORAGE") {
            match val.to_lowercase().as_str() {
                "sled" => self.server.storage = StorageBackend::Sled,
                "memory" => self.server.storage = StorageBackend::Memory,
                other => eprintln!("Warning: unknown FF_STORAGE value: {}", other),
            }
        }

        // FF_NODE_ID — creates cluster config if not present
        if let Ok(val) = env::var("FF_NODE_ID") {
            if let Ok(node_id) = val.parse::<u64>() {
                let cluster = self.cluster.get_or_insert_with(|| ClusterConfig {
                    node_id,
                    grpc_port: default_grpc_port(),
                    peers: Vec::new(),
                    election_timeout_ms: default_election_timeout(),
                    heartbeat_interval_ms: default_heartbeat_interval(),
                    snapshot_threshold: default_snapshot_threshold(),
                });
                cluster.node_id = node_id;
            }
        }

        // FF_GRPC_PORT
        if let Ok(val) = env::var("FF_GRPC_PORT") {
            if let Ok(port) = val.parse::<u16>() {
                if let Some(ref mut cluster) = self.cluster {
                    cluster.grpc_port = port;
                }
            }
        }

        // FF_PEERS — format: "2:host:9090,3:host:9090"
        if let Ok(val) = env::var("FF_PEERS") {
            if let Some(ref mut cluster) = self.cluster {
                let mut peers = Vec::new();
                for entry in val.split(',') {
                    let entry = entry.trim();
                    if entry.is_empty() {
                        continue;
                    }
                    // Split into at most 2 parts: id and addr
                    if let Some((id_str, addr)) = entry.split_once(':') {
                        if let Ok(id) = id_str.parse::<u64>() {
                            peers.push(PeerConfig {
                                id,
                                addr: addr.to_string(),
                            });
                        }
                    }
                }
                cluster.peers = peers;
            }
        }

        // FF_ROOT_READ_TOKENS
        if let Ok(val) = env::var("FF_ROOT_READ_TOKENS") {
            self.root.read_tokens = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // FF_ROOT_WRITE_TOKENS
        if let Ok(val) = env::var("FF_ROOT_WRITE_TOKENS") {
            self.root.write_tokens = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // FF_NS_{NAME}_READ_TOKENS and FF_NS_{NAME}_WRITE_TOKENS
        for (key, val) in env::vars() {
            if let Some(rest) = key.strip_prefix("FF_NS_") {
                if let Some(name) = rest.strip_suffix("_READ_TOKENS") {
                    let ns_name = name.to_lowercase();
                    let ns = self.namespaces.entry(ns_name).or_default();
                    ns.read_tokens = val
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                } else if let Some(name) = rest.strip_suffix("_WRITE_TOKENS") {
                    let ns_name = name.to_lowercase();
                    let ns = self.namespaces.entry(ns_name).or_default();
                    ns.write_tokens = val
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
        }
    }
}
