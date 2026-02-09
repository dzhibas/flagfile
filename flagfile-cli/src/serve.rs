use std::collections::HashMap;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use flagfile_lib::ast::Atom;
use flagfile_lib::eval::Context;
use flagfile_lib::parse_flagfile::{parse_flagfile, FlagReturn, Rule};
use notify::{EventKind, RecursiveMode, Watcher};
use tokio::sync::RwLock;

use crate::evaluate_flag;

// --- OFREP request/response types ---

#[derive(serde::Deserialize)]
struct OFREPEvalRequest {
    context: Option<HashMap<String, serde_json::Value>>,
}

#[derive(serde::Serialize)]
struct OFREPEvalSuccess {
    key: String,
    reason: String,
    variant: String,
    value: serde_json::Value,
    metadata: serde_json::Value,
}

#[derive(serde::Serialize)]
struct OFREPEvalError {
    key: String,
    #[serde(rename = "errorCode")]
    error_code: String,
    #[serde(rename = "errorDetails")]
    error_details: String,
}

#[derive(serde::Serialize)]
struct OFREPBulkResponse {
    flags: Vec<serde_json::Value>,
}

#[derive(serde::Deserialize, Default)]
struct ServeConfig {
    port: Option<u16>,
    flagfile: Option<String>,
}

pub struct FlagStore {
    pub flagfile_content: String,
    pub flags: HashMap<String, Vec<Rule>>,
}

pub struct AppState {
    pub store: RwLock<FlagStore>,
}

async fn handle_flagfile(State(state): State<Arc<AppState>>) -> Response {
    let store = state.store.read().await;
    (
        StatusCode::OK,
        [("content-type", "text/plain")],
        store.flagfile_content.clone(),
    )
        .into_response()
}

async fn handle_eval(
    State(state): State<Arc<AppState>>,
    Path(flag_name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let store = state.store.read().await;
    let plain = params
        .get("ff_output")
        .map(|v| v == "plain")
        .unwrap_or(false);

    let Some(rules) = store.flags.get(&flag_name) else {
        if plain {
            return (StatusCode::NOT_FOUND, "flag not found").into_response();
        }
        return (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "flag not found", "flag": flag_name})),
        )
            .into_response();
    };

    let context: Context = params
        .iter()
        .filter(|(k, _)| k.as_str() != "ff_output")
        .map(|(k, v)| (k.as_str(), Atom::from(v.as_str())))
        .collect();

    match evaluate_flag(rules, &context) {
        Some(FlagReturn::OnOff(val)) => {
            if plain {
                return (StatusCode::OK, val.to_string()).into_response();
            }
            (
                StatusCode::OK,
                axum::Json(serde_json::json!({"flag": flag_name, "value": val})),
            )
                .into_response()
        }
        Some(FlagReturn::Json(val)) => {
            if plain {
                return (StatusCode::OK, val.to_string()).into_response();
            }
            (
                StatusCode::OK,
                axum::Json(serde_json::json!({"flag": flag_name, "value": val})),
            )
                .into_response()
        }
        Some(FlagReturn::Integer(val)) => {
            if plain {
                return (StatusCode::OK, val.to_string()).into_response();
            }
            (
                StatusCode::OK,
                axum::Json(serde_json::json!({"flag": flag_name, "value": val})),
            )
                .into_response()
        }
        Some(FlagReturn::Str(val)) => {
            if plain {
                return (StatusCode::OK, val.clone()).into_response();
            }
            (
                StatusCode::OK,
                axum::Json(serde_json::json!({"flag": flag_name, "value": val})),
            )
                .into_response()
        }
        None => {
            if plain {
                return (StatusCode::UNPROCESSABLE_ENTITY, "no rule matched").into_response();
            }
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                axum::Json(serde_json::json!({"error": "no rule matched", "flag": flag_name})),
            )
                .into_response()
        }
    }
}

/// Convert OFREP context (JSON values) to flagfile Context (string-based Atoms).
fn build_context_from_ofrep(raw: &HashMap<String, serde_json::Value>) -> HashMap<String, String> {
    raw.iter()
        .map(|(k, v)| {
            let s = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                other => other.to_string(),
            };
            (k.clone(), s)
        })
        .collect()
}

/// Evaluate a flag and return the result along with a reason string.
/// Returns (FlagReturn, reason) where reason is "TARGETING_MATCH" or "DEFAULT".
fn evaluate_flag_with_reason(
    rules: &[Rule],
    context: &Context,
) -> Option<(FlagReturn, &'static str)> {
    for rule in rules {
        match rule {
            Rule::BoolExpressionValue(expr, return_val) => {
                if let Ok(true) = flagfile_lib::eval::eval(expr, context) {
                    return Some((return_val.clone(), "TARGETING_MATCH"));
                }
            }
            Rule::Value(return_val) => {
                return Some((return_val.clone(), "DEFAULT"));
            }
        }
    }
    None
}

fn flag_return_to_ofrep(key: &str, ret: &FlagReturn, reason: &str) -> OFREPEvalSuccess {
    match ret {
        FlagReturn::OnOff(val) => OFREPEvalSuccess {
            key: key.to_string(),
            reason: reason.to_string(),
            variant: val.to_string(),
            value: serde_json::Value::Bool(*val),
            metadata: serde_json::json!({}),
        },
        FlagReturn::Json(val) => OFREPEvalSuccess {
            key: key.to_string(),
            reason: reason.to_string(),
            variant: "json".to_string(),
            value: val.clone(),
            metadata: serde_json::json!({}),
        },
        FlagReturn::Integer(val) => OFREPEvalSuccess {
            key: key.to_string(),
            reason: reason.to_string(),
            variant: val.to_string(),
            value: serde_json::json!(*val),
            metadata: serde_json::json!({}),
        },
        FlagReturn::Str(val) => OFREPEvalSuccess {
            key: key.to_string(),
            reason: reason.to_string(),
            variant: val.clone(),
            value: serde_json::Value::String(val.clone()),
            metadata: serde_json::json!({}),
        },
    }
}

async fn handle_ofrep_single(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(body): Json<OFREPEvalRequest>,
) -> Response {
    let store = state.store.read().await;

    let Some(rules) = store.flags.get(&key) else {
        return (
            StatusCode::NOT_FOUND,
            Json(OFREPEvalError {
                key: key.clone(),
                error_code: "FLAG_NOT_FOUND".to_string(),
                error_details: format!("Flag '{}' was not found", key),
            }),
        )
            .into_response();
    };

    let string_ctx = body
        .context
        .as_ref()
        .map(build_context_from_ofrep)
        .unwrap_or_default();

    let context: Context = string_ctx
        .iter()
        .map(|(k, v)| (k.as_str(), Atom::from(v.as_str())))
        .collect();

    match evaluate_flag_with_reason(rules, &context) {
        Some((ret, reason)) => {
            let success = flag_return_to_ofrep(&key, &ret, reason);
            (StatusCode::OK, Json(success)).into_response()
        }
        None => {
            let success = OFREPEvalSuccess {
                key: key.clone(),
                reason: "DEFAULT".to_string(),
                variant: "false".to_string(),
                value: serde_json::Value::Bool(false),
                metadata: serde_json::json!({}),
            };
            (StatusCode::OK, Json(success)).into_response()
        }
    }
}

async fn handle_ofrep_bulk(
    State(state): State<Arc<AppState>>,
    Json(body): Json<OFREPEvalRequest>,
) -> Response {
    let store = state.store.read().await;

    let string_ctx = body
        .context
        .as_ref()
        .map(build_context_from_ofrep)
        .unwrap_or_default();

    let context: Context = string_ctx
        .iter()
        .map(|(k, v)| (k.as_str(), Atom::from(v.as_str())))
        .collect();

    let mut flags = Vec::new();
    for (key, rules) in store.flags.iter() {
        let result = match evaluate_flag_with_reason(rules, &context) {
            Some((ret, reason)) => flag_return_to_ofrep(key, &ret, reason),
            None => OFREPEvalSuccess {
                key: key.clone(),
                reason: "DEFAULT".to_string(),
                variant: "false".to_string(),
                value: serde_json::Value::Bool(false),
                metadata: serde_json::json!({}),
            },
        };
        flags.push(serde_json::to_value(result).unwrap());
    }

    (StatusCode::OK, Json(OFREPBulkResponse { flags })).into_response()
}

fn parse_flags(content: &str) -> Option<HashMap<String, Vec<Rule>>> {
    let (remainder, flag_values) = match parse_flagfile(content) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Warning: reload parse error: {}", e);
            return None;
        }
    };

    if !remainder.trim().is_empty() {
        eprintln!(
            "Warning: reload failed: unexpected content near: {}",
            remainder.trim().lines().next().unwrap_or("")
        );
        return None;
    }

    let mut flags: HashMap<String, Vec<Rule>> = HashMap::new();
    for fv in &flag_values {
        for (name, rules) in fv.iter() {
            flags.insert(name.to_string(), rules.clone());
        }
    }
    Some(flags)
}

async fn watch_flagfile(state: Arc<AppState>, path: PathBuf) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                let _ = tx.try_send(());
            }
        }
    })
    .unwrap_or_else(|e| {
        eprintln!("Failed to create file watcher: {}", e);
        process::exit(1);
    });

    let watch_path = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    watcher
        .watch(&watch_path, RecursiveMode::NonRecursive)
        .unwrap_or_else(|e| {
            eprintln!("Failed to watch {}: {}", watch_path.display(), e);
            process::exit(1);
        });

    println!("Watching {} for changes", path.display());

    // Keep watcher alive for the lifetime of this task
    let _watcher = watcher;

    loop {
        // Wait for a change notification
        if rx.recv().await.is_none() {
            break;
        }

        // Debounce: wait a bit and drain any extra events
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        while rx.try_recv().is_ok() {}

        // Re-read and re-parse
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: failed to read {}: {}", path.display(), e);
                continue;
            }
        };

        match parse_flags(&content) {
            Some(flags) => {
                let mut store = state.store.write().await;
                store.flagfile_content = content;
                store.flags = flags;
                println!("Flagfile reloaded successfully");
            }
            None => {
                // parse_flags already printed the warning
            }
        }
    }
}

pub async fn run_serve(flagfile_arg: Option<String>, port_arg: Option<u16>, config_path: &str) {
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

    // Read and parse flagfile
    let flagfile_content = match std::fs::read_to_string(&flagfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            process::exit(1);
        }
    };

    let flags = match parse_flags(&flagfile_content) {
        Some(flags) => flags,
        None => {
            eprintln!("Initial parsing of {} failed", flagfile_path);
            process::exit(1);
        }
    };

    let state = Arc::new(AppState {
        store: RwLock::new(FlagStore {
            flagfile_content,
            flags,
        }),
    });

    // Spawn file watcher
    let watcher_state = Arc::clone(&state);
    let watcher_path = PathBuf::from(&flagfile_path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(&flagfile_path));
    tokio::spawn(watch_flagfile(watcher_state, watcher_path));

    let app = Router::new()
        .route("/flagfile", get(handle_flagfile))
        .route("/eval/{flag_name}", get(handle_eval))
        .route("/ofrep/v1/evaluate/flags/{key}", post(handle_ofrep_single))
        .route("/ofrep/v1/evaluate/flags", post(handle_ofrep_bulk))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("Serving {} on http://{}", flagfile_path, addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to bind to {}: {}", addr, e);
            process::exit(1);
        });

    axum::serve(listener, app).await.unwrap_or_else(|e| {
        eprintln!("Server error: {}", e);
        process::exit(1);
    });
}
