use std::collections::HashMap;
use std::sync::Arc;

use flagfile_lib::ast::FlagMetadata;
use flagfile_lib::eval::Segments;
use flagfile_lib::parse_flagfile::Rule;
use tokio::sync::RwLock;

use super::config::{FfServerConfig, NamespaceConfig};
use super::sse::SseBroadcaster;
use super::store::{self, ROOT_NAMESPACE};

pub type ParsedFlags = (
    HashMap<String, Vec<Rule>>,
    HashMap<String, FlagMetadata>,
    Segments,
);

/// Parsed flagfile content for a single namespace (or the single-tenant default).
pub struct ParsedNamespace {
    pub flagfile_content: String,
    pub flags: HashMap<String, Vec<Rule>>,
    pub metadata: HashMap<String, FlagMetadata>,
    pub segments: Segments,
    pub env: Option<String>,
}

/// Shared application state for the HTTP server.
///
/// In single-tenant mode (no ff-server.toml), only the `ROOT_NAMESPACE` key
/// exists in `namespaces` and auth is disabled.
///
/// In multi-tenant mode, each configured namespace has its own entry and
/// requests are authenticated via bearer tokens.
pub struct AppState {
    /// Parsed flags per namespace. Single-tenant uses `ROOT_NAMESPACE`.
    pub namespaces: RwLock<HashMap<String, ParsedNamespace>>,
    /// Server configuration (tokens, cluster).
    pub config: Arc<FfServerConfig>,
    /// SSE broadcaster for live flag updates.
    pub broadcaster: Arc<SseBroadcaster>,
    /// Persistent storage backend (Some in multi-tenant mode).
    pub persistent_store: Option<Arc<dyn store::FlagStore + Send + Sync>>,
    /// Whether the server is running in multi-tenant mode.
    pub multi_tenant: bool,
    /// Raft consensus handle (cluster mode only).
    pub raft_handle: std::sync::OnceLock<super::raft::node::RaftHandle>,
    /// Raft gRPC transport for forwarding writes to the leader.
    pub raft_transport: std::sync::OnceLock<Arc<super::raft::transport::RaftTransport>>,
}

impl AppState {
    /// Get the namespace config for a given namespace key.
    /// Returns the root config for `ROOT_NAMESPACE`, or the named namespace config.
    /// In single-tenant mode, returns a permissive default (no tokens required).
    /// Returns `None` for unconfigured namespaces in multi-tenant mode — callers
    /// must deny access when this returns `None`.
    pub fn namespace_config(&self, namespace: &str) -> Option<NamespaceConfig> {
        if !self.multi_tenant {
            return Some(NamespaceConfig::default());
        }
        if namespace == ROOT_NAMESPACE {
            Some(self.config.root.clone())
        } else {
            self.config.namespaces.get(namespace).cloned()
        }
    }

    /// Resolve the namespace key from a path parameter.
    /// `None` means root namespace.
    pub fn resolve_namespace(ns_param: Option<&str>) -> &str {
        ns_param.unwrap_or(ROOT_NAMESPACE)
    }
}

