pub mod auth;
pub mod config;
pub mod metrics;
mod ofrep;
pub mod raft;
mod routes;
pub mod sse;
pub mod state;
pub mod store;
mod watch;

use std::collections::HashMap;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::compression::CompressionLayer;

use self::config::{FfServerConfig, StorageBackend};
use self::metrics::{handle_health_check, handle_metrics, handle_readyz, track_metrics};
use self::ofrep::{handle_ofrep_bulk, handle_ofrep_single};
use self::routes::{
    handle_eval, handle_events, handle_flagfile, handle_flagfile_hash, handle_health,
    handle_put_flagfile,
};
use self::sse::SseBroadcaster;
use self::state::{AppState, ParsedNamespace};
use self::store::ROOT_NAMESPACE;
use self::watch::parse_flags;

/// Legacy simple config for single-tenant mode (ff.toml without [server] section).
#[derive(serde::Deserialize, Default, Debug)]
struct SimpleServeConfig {
    port: Option<u16>,
    hostname: Option<String>,
    flagfile: Option<String>,
    env: Option<String>,
}

/// Detect whether the config file is a full ff-server.toml (has [server] or [root] sections)
/// or a simple ff.toml.
fn is_multi_tenant_config(path: &str) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    content.contains("[server]")
        || content.contains("[root]")
        || content.contains("[namespaces")
        || content.contains("[cluster]")
}

pub async fn run_serve(
    flagfile_arg: Option<String>,
    port_arg: Option<u16>,
    hostname_arg: Option<String>,
    watch: bool,
    config_path: &str,
    env_arg: Option<String>,
) {
    if is_multi_tenant_config(config_path) {
        run_serve_multi_tenant(config_path, port_arg, hostname_arg, env_arg).await;
    } else {
        run_serve_single_tenant(flagfile_arg, port_arg, hostname_arg, watch, config_path, env_arg)
            .await;
    }
}

// ── Single-tenant mode ──────────────────────────────────────
// Backward compatible: `ff serve -f Flagfile --watch`

async fn run_serve_single_tenant(
    flagfile_arg: Option<String>,
    port_arg: Option<u16>,
    hostname_arg: Option<String>,
    watch: bool,
    config_path: &str,
    env_arg: Option<String>,
) {
    let config: SimpleServeConfig = std::fs::read_to_string(config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_default();

    let flagfile_path = flagfile_arg
        .or(config.flagfile)
        .unwrap_or_else(|| "Flagfile".to_string());
    let port = port_arg.or(config.port).unwrap_or(8080);
    let hostname = hostname_arg
        .or(config.hostname)
        .unwrap_or_else(|| "0.0.0.0".to_string());
    let env = env_arg.or(config.env);

    let flagfile_content = match std::fs::read_to_string(&flagfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            process::exit(1);
        }
    };

    let (flags, metadata, segments) = match parse_flags(&flagfile_content) {
        Some(result) => result,
        None => {
            eprintln!("Initial parsing of {} failed", flagfile_path);
            process::exit(1);
        }
    };

    // Record startup flags metric
    metrics::metrics().flags_total.with_label_values(&[ROOT_NAMESPACE]).set(flags.len() as i64);

    let mut namespaces = HashMap::new();
    namespaces.insert(
        ROOT_NAMESPACE.to_string(),
        ParsedNamespace {
            flagfile_content,
            flags,
            metadata,
            segments,
            env: env.clone(),
        },
    );

    let broadcaster = Arc::new(SseBroadcaster::new());

    let state = Arc::new(AppState {
        namespaces: tokio::sync::RwLock::new(namespaces),
        config: Arc::new(FfServerConfig::default()),
        broadcaster: Arc::clone(&broadcaster),
        persistent_store: None,
        multi_tenant: false,
        raft_handle: std::sync::OnceLock::new(),
        raft_transport: std::sync::OnceLock::new(),
    });

    // Spawn file watcher if --watch is enabled
    if watch {
        let watcher_state = Arc::clone(&state);
        let watcher_broadcaster = Arc::clone(&broadcaster);
        let watcher_path = PathBuf::from(&flagfile_path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&flagfile_path));
        let watcher_env = env.clone();
        tokio::spawn(watch_flagfile_new(
            watcher_state,
            watcher_broadcaster,
            watcher_path,
            watcher_env,
        ));
    }

    let app = build_single_tenant_router(Arc::clone(&state));

    let addr = format!("{}:{}", hostname, port);
    if let Some(ref env) = env {
        println!(
            "Serving {} on http://{} (env: {})",
            flagfile_path, addr, env
        );
    } else {
        println!("Serving {} on http://{}", flagfile_path, addr);
    }

    serve_with_shutdown(app, &addr, broadcaster, state).await;
}

// ── Multi-tenant mode ───────────────────────────────────────
// Config-based: `ff serve --config ff-server.toml`

async fn run_serve_multi_tenant(
    config_path: &str,
    port_arg: Option<u16>,
    hostname_arg: Option<String>,
    env_arg: Option<String>,
) {
    let mut server_config = FfServerConfig::load(config_path);
    server_config.apply_env_overrides();

    let port = port_arg.unwrap_or(server_config.server.port);
    let hostname = hostname_arg.unwrap_or_else(|| server_config.server.hostname.clone());

    // Initialize persistent storage
    let persistent_store: Arc<dyn store::FlagStore + Send + Sync> =
        match server_config.server.storage {
            StorageBackend::Sled => {
                metrics::metrics().storage_backend.with_label_values(&["sled"]).set(1);
                match store::sled_store::SledStore::open(&server_config.server.data_dir) {
                    Ok(s) => Arc::new(s),
                    Err(e) => {
                        eprintln!("Failed to open sled storage: {}", e);
                        process::exit(1);
                    }
                }
            }
            StorageBackend::Memory => {
                metrics::metrics().storage_backend.with_label_values(&["memory"]).set(1);
                Arc::new(store::memory::MemoryStore::new())
            }
        };

    // Load existing flagfiles from persistent store into parsed namespaces
    let mut namespaces = HashMap::new();
    let stored_ns: Vec<String> = persistent_store.list_namespaces().await;
    for ns_key in &stored_ns {
        if let Some(content_bytes) = persistent_store.get_flagfile(ns_key).await {
            if let Ok(content) = String::from_utf8(content_bytes) {
                if let Some((flags, metadata, segments)) = parse_flags(&content) {
                    namespaces.insert(
                        ns_key.clone(),
                        ParsedNamespace {
                            flagfile_content: content,
                            flags,
                            metadata,
                            segments,
                            env: env_arg.clone(),
                        },
                    );
                }
            }
        }
    }

    // Record flags_total per namespace at startup
    for (ns_key, ns_data) in &namespaces {
        metrics::metrics().flags_total.with_label_values(&[ns_key]).set(ns_data.flags.len() as i64);
    }

    let broadcaster = Arc::new(SseBroadcaster::new());

    let state = Arc::new(AppState {
        namespaces: tokio::sync::RwLock::new(namespaces),
        config: Arc::new(server_config),
        broadcaster: Arc::clone(&broadcaster),
        persistent_store: Some(Arc::clone(&persistent_store)),
        multi_tenant: true,
        raft_handle: std::sync::OnceLock::new(),
        raft_transport: std::sync::OnceLock::new(),
    });

    // Start Raft consensus node + gRPC server if cluster is configured.
    if let Some(ref cluster_cfg) = state.config.cluster {
        use self::raft::node::run_raft_node;
        use self::raft::state_machine::RaftStateMachine;
        use self::raft::storage::MemRaftStorage;
        use self::raft::transport::{RaftGrpcService, RaftTransport};

        // Collect all voter IDs (this node + peers).
        let mut voter_ids: Vec<u64> = cluster_cfg.peers.iter().map(|p| p.id).collect();
        if !voter_ids.contains(&cluster_cfg.node_id) {
            voter_ids.push(cluster_cfg.node_id);
        }
        voter_ids.sort();

        let storage = MemRaftStorage::new(voter_ids);
        let transport = Arc::new(RaftTransport::new(cluster_cfg.peers.clone()));
        let state_machine = Arc::new(RaftStateMachine::new(
            Arc::clone(&persistent_store),
            Arc::clone(&state),
        ));

        let (handle, raft_msg_tx) =
            run_raft_node(cluster_cfg, storage, Arc::clone(&transport), state_machine).await;

        // Store handle and transport in AppState for route handlers.
        let _ = state.raft_handle.set(handle.clone());
        let _ = state.raft_transport.set(Arc::clone(&transport));

        // Spawn gRPC server for inter-node Raft communication.
        let grpc_port = cluster_cfg.grpc_port;
        let grpc_service = RaftGrpcService::new(raft_msg_tx, handle);

        tokio::spawn(async move {
            use self::raft::transport::proto::raft_service_server::RaftServiceServer;

            let grpc_addr = format!("0.0.0.0:{}", grpc_port)
                .parse()
                .expect("invalid gRPC address");

            println!(
                "Raft gRPC server listening on 0.0.0.0:{}",
                grpc_port
            );

            if let Err(e) = tonic::transport::Server::builder()
                .add_service(RaftServiceServer::new(grpc_service))
                .serve(grpc_addr)
                .await
            {
                eprintln!("gRPC server error: {}", e);
            }
        });
    }

    let app = build_multi_tenant_router(Arc::clone(&state));

    let addr = format!("{}:{}", hostname, port);
    let ns_count = stored_ns.len();
    println!(
        "Serving multi-tenant on http://{} ({} namespaces loaded)",
        addr, ns_count
    );

    serve_with_shutdown(app, &addr, broadcaster, state).await;
}

// ── Router builders ─────────────────────────────────────────

fn build_single_tenant_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(handle_health))
        .route("/flagfile", get(handle_flagfile).put(handle_put_flagfile))
        .route("/flagfile/hash", get(handle_flagfile_hash))
        .route("/events", get(handle_events))
        .route("/v1/eval/{flag_name}", get(handle_eval))
        .route(
            "/ofrep/v1/evaluate/flags/{key}",
            post(handle_ofrep_single),
        )
        .route("/ofrep/v1/evaluate/flags", post(handle_ofrep_bulk))
        .route("/metrics", get(handle_metrics))
        .layer(axum::middleware::from_fn(track_metrics))
        .layer(CompressionLayer::new())
        .with_state(state)
}

fn build_multi_tenant_router(state: Arc<AppState>) -> Router {
    // Root namespace routes
    let root_routes: Router<Arc<AppState>> = Router::new()
        .route("/flagfile", get(handle_flagfile).put(handle_put_flagfile))
        .route("/flagfile/hash", get(handle_flagfile_hash))
        .route("/events", get(handle_events))
        .route("/v1/eval/{flag_name}", get(handle_eval))
        .route(
            "/ofrep/v1/evaluate/flags/{key}",
            post(handle_ofrep_single),
        )
        .route("/ofrep/v1/evaluate/flags", post(handle_ofrep_bulk));

    // Namespaced routes: /ns/{namespace}/...
    let ns_routes = Router::new()
        .route(
            "/ns/{namespace}/flagfile",
            get(handle_flagfile_ns).put(handle_put_flagfile_ns),
        )
        .route("/ns/{namespace}/flagfile/hash", get(handle_flagfile_hash_ns))
        .route("/ns/{namespace}/events", get(handle_events_ns))
        .route(
            "/ns/{namespace}/v1/eval/{flag_name}",
            get(handle_eval_ns),
        );

    // Observability (no auth)
    let obs_routes = Router::new()
        .route("/health", get(handle_health_check))
        .route("/readyz", get(handle_readyz))
        .route("/metrics", get(handle_metrics));

    Router::new()
        .merge(root_routes)
        .merge(ns_routes)
        .merge(obs_routes)
        .layer(axum::middleware::from_fn(track_metrics))
        .layer(CompressionLayer::new())
        .with_state(state)
}

// ── Namespace wrapper handlers ──────────────────────────────
// These extract the namespace from the path and delegate to the core handlers.

async fn handle_flagfile_ns(
    state: axum::extract::State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(namespace): axum::extract::Path<String>,
) -> axum::response::Response {
    handle_flagfile(state, headers, Some(axum::extract::Path(namespace))).await
}

async fn handle_flagfile_hash_ns(
    state: axum::extract::State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(namespace): axum::extract::Path<String>,
) -> axum::response::Response {
    handle_flagfile_hash(state, headers, Some(axum::extract::Path(namespace))).await
}

async fn handle_put_flagfile_ns(
    state: axum::extract::State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(namespace): axum::extract::Path<String>,
    body: String,
) -> axum::response::Response {
    handle_put_flagfile(state, headers, Some(axum::extract::Path(namespace)), body).await
}

async fn handle_events_ns(
    state: axum::extract::State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(namespace): axum::extract::Path<String>,
) -> axum::response::Response {
    handle_events(state, headers, Some(axum::extract::Path(namespace))).await
}

async fn handle_eval_ns(
    state: axum::extract::State<Arc<AppState>>,
    axum::extract::Path((namespace, flag_name)): axum::extract::Path<(String, String)>,
    query: axum::extract::Query<HashMap<String, String>>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let params = routes::EvalParams {
        flag_name,
        namespace: Some(namespace),
    };
    handle_eval(state, axum::extract::Path(params), query, headers).await
}

// ── File watcher (new state format) ─────────────────────────

async fn watch_flagfile_new(
    state: Arc<AppState>,
    broadcaster: Arc<SseBroadcaster>,
    path: PathBuf,
    env: Option<String>,
) {
    use std::time::Duration;

    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use sha1::Digest;

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    let parent = path.parent().unwrap_or(&path).to_path_buf();

    let mut watcher = RecommendedWatcher::new(
        move |event: Result<notify::Event, notify::Error>| {
            if let Ok(event) = event {
                let _ = tx.blocking_send(event);
            }
        },
        Config::default(),
    )
    .expect("failed to create file watcher");

    watcher
        .watch(&parent, RecursiveMode::NonRecursive)
        .expect("failed to watch directory");

    let mut last_reload = std::time::Instant::now();

    while let Some(event) = rx.recv().await {
        let relevant = event.paths.iter().any(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("Flagfile"))
                .unwrap_or(false)
        });

        if !relevant {
            continue;
        }

        // Debounce: ignore events within 500ms of last reload
        if last_reload.elapsed() < Duration::from_millis(500) {
            continue;
        }
        last_reload = std::time::Instant::now();

        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                if let Some((flags, metadata, segments)) = parse_flags(&content) {
                    let flags_count = flags.len() as u64;
                    let mut hasher = sha1::Sha1::new();
                    hasher.update(content.as_bytes());
                    let hash = format!("{:x}", hasher.finalize());

                    let mut namespaces = state.namespaces.write().await;
                    namespaces.insert(
                        ROOT_NAMESPACE.to_string(),
                        ParsedNamespace {
                            flagfile_content: content,
                            flags,
                            metadata,
                            segments,
                            env: env.clone(),
                        },
                    );
                    drop(namespaces);

                    broadcaster
                        .broadcast(
                            ROOT_NAMESPACE,
                            sse::FlagUpdateEvent {
                                hash,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                flags_count,
                            },
                        )
                        .await;

                    println!("Flagfile reloaded ({} flags)", flags_count);
                } else {
                    eprintln!("Warning: failed to parse updated Flagfile");
                }
            }
            Err(e) => {
                eprintln!("Warning: failed to read {}: {}", path.display(), e);
            }
        }
    }
}

// ── Shared server startup ───────────────────────────────────

async fn serve_with_shutdown(
    app: Router,
    addr: &str,
    broadcaster: Arc<SseBroadcaster>,
    state: Arc<AppState>,
) {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to bind to {}: {}", addr, e);
            process::exit(1);
        });

    let shutdown = async move {
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler");

        #[cfg(unix)]
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }

        #[cfg(not(unix))]
        ctrl_c.await.ok();

        println!("Shutdown signal received");

        // Attempt Raft leadership transfer before shutting down.
        if let Some(handle) = state.raft_handle.get() {
            if handle.is_leader() {
                println!("Transferring Raft leadership...");
                match handle.transfer_leader().await {
                    Ok(()) => {
                        // Poll is_leader() until we're no longer leader, up to 5s.
                        let deadline =
                            tokio::time::Instant::now() + std::time::Duration::from_secs(5);
                        while handle.is_leader()
                            && tokio::time::Instant::now() < deadline
                        {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                        if handle.is_leader() {
                            println!("Leadership transfer timed out, proceeding with shutdown");
                        } else {
                            println!("Leadership transferred successfully");
                        }
                    }
                    Err(e) => {
                        println!("Leadership transfer skipped: {}", e);
                    }
                }
            }
        }

        println!("Notifying SSE clients...");
        broadcaster.shutdown();
        // Brief pause so SSE clients receive the shutdown event before
        // connections are torn down.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        println!("Finishing in-flight requests...");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Server error: {}", e);
            process::exit(1);
        });

    println!("Server stopped");
}
