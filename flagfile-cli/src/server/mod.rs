mod ofrep;
mod routes;
mod state;
mod watch;

use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tokio::sync::RwLock;
use tower_http::compression::CompressionLayer;

use self::ofrep::{handle_ofrep_bulk, handle_ofrep_single};
use self::routes::{handle_eval, handle_flagfile, handle_health};
use self::state::{AppState, FlagStore};
use self::watch::{parse_flags, watch_flagfile};

#[derive(serde::Deserialize, Default, Debug)]
struct ServeConfig {
    port: Option<u16>,
    hostname: Option<String>,
    flagfile: Option<String>,
    env: Option<String>,
}

pub async fn run_serve(
    flagfile_arg: Option<String>,
    port_arg: Option<u16>,
    hostname_arg: Option<String>,
    watch: bool,
    config_path: &str,
    env_arg: Option<String>,
) {
    // Load config from file if it exists
    let config: ServeConfig = std::fs::read_to_string(config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_default();

    // CLI args override config file values, which override defaults
    let flagfile_path = flagfile_arg
        .or(config.flagfile)
        .unwrap_or_else(|| "Flagfile".to_string());
    let port = port_arg.or(config.port).unwrap_or(8080);
    let hostname = hostname_arg
        .or(config.hostname)
        .unwrap_or_else(|| "0.0.0.0".to_string());
    let env = env_arg.or(config.env);

    // Read and parse flagfile
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

    let state = Arc::new(AppState {
        store: RwLock::new(FlagStore {
            flagfile_content,
            flags,
            metadata,
            segments,
            env: env.clone(),
        }),
    });

    // Spawn file watcher if --watch is enabled
    if watch {
        let watcher_state = Arc::clone(&state);
        let watcher_path = PathBuf::from(&flagfile_path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&flagfile_path));
        tokio::spawn(watch_flagfile(watcher_state, watcher_path));
    }

    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/flagfile", get(handle_flagfile))
        .route("/v1/eval/{flag_name}", get(handle_eval))
        .route("/ofrep/v1/evaluate/flags/{key}", post(handle_ofrep_single))
        .route("/ofrep/v1/evaluate/flags", post(handle_ofrep_bulk))
        .layer(CompressionLayer::new())
        .with_state(state);

    let addr = format!("{}:{}", hostname, port);
    if let Some(ref env) = env {
        println!(
            "Serving {} on http://{} (env: {})",
            flagfile_path, addr, env
        );
    } else {
        println!("Serving {} on http://{}", flagfile_path, addr);
    }

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to bind to {}: {}", addr, e);
            process::exit(1);
        });

    let shutdown = async {
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

        println!("Shutdown signal received, finishing in-flight requests...");
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
