use std::collections::HashMap;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use flagfile_lib::ast::{Atom, FlagMetadata};
use flagfile_lib::eval::{eval_with_segments, Context, Segments};
use flagfile_lib::parse_flagfile::{parse_flagfile_with_segments, FlagReturn, Rule};
use notify::{EventKind, RecursiveMode, Watcher};
use tokio::sync::RwLock;

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

#[derive(serde::Deserialize, Default, Debug)]
struct ServeConfig {
    port: Option<u16>,
    flagfile: Option<String>,
    env: Option<String>,
}

pub struct FlagStore {
    pub flagfile_content: String,
    pub flags: HashMap<String, Vec<Rule>>,
    pub metadata: HashMap<String, FlagMetadata>,
    pub segments: Segments,
    pub env: Option<String>,
}

pub struct AppState {
    pub store: RwLock<FlagStore>,
}

async fn handle_health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let store = state.store.read().await;
    Json(serde_json::json!({
        "status": "ok",
        "flags_loaded": store.flags.len()
    }))
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

    if !store.flags.contains_key(&flag_name) {
        if plain {
            return (StatusCode::NOT_FOUND, "flag not found").into_response();
        }
        return (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "flag not found", "flag": flag_name})),
        )
            .into_response();
    }

    let context: Context = params
        .iter()
        .filter(|(k, _)| k.as_str() != "ff_output")
        .map(|(k, v)| (k.as_str(), Atom::from(v.as_str())))
        .collect();

    match evaluate_flag_with_reason(
        &flag_name,
        &context,
        &store.flags,
        &store.metadata,
        &store.segments,
        store.env.as_deref(),
    ) {
        Some((FlagReturn::OnOff(val), _)) => {
            if plain {
                return (StatusCode::OK, val.to_string()).into_response();
            }
            (
                StatusCode::OK,
                axum::Json(serde_json::json!({"flag": flag_name, "value": val})),
            )
                .into_response()
        }
        Some((FlagReturn::Json(val), _)) => {
            if plain {
                return (StatusCode::OK, val.to_string()).into_response();
            }
            (
                StatusCode::OK,
                axum::Json(serde_json::json!({"flag": flag_name, "value": val})),
            )
                .into_response()
        }
        Some((FlagReturn::Integer(val), _)) => {
            if plain {
                return (StatusCode::OK, val.to_string()).into_response();
            }
            (
                StatusCode::OK,
                axum::Json(serde_json::json!({"flag": flag_name, "value": val})),
            )
                .into_response()
        }
        Some((FlagReturn::Str(val), _)) => {
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

/// Evaluate rules and return the result along with a reason string.
/// Handles all rule types including `EnvRule`.
fn evaluate_rules_with_reason(
    rules: &[Rule],
    context: &Context,
    flag_name: Option<&str>,
    segments: &Segments,
    env: Option<&str>,
) -> Option<(FlagReturn, &'static str)> {
    for rule in rules {
        match rule {
            Rule::BoolExpressionValue(expr, return_val) => {
                if let Ok(true) = eval_with_segments(expr, context, flag_name, segments) {
                    return Some((return_val.clone(), "TARGETING_MATCH"));
                }
            }
            Rule::Value(return_val) => {
                return Some((return_val.clone(), "DEFAULT"));
            }
            Rule::EnvRule {
                env: rule_env,
                rules: sub_rules,
            } => {
                if env == Some(rule_env.as_str()) {
                    let result =
                        evaluate_rules_with_reason(sub_rules, context, flag_name, segments, env);
                    if result.is_some() {
                        return result;
                    }
                }
            }
        }
    }
    None
}

/// Evaluate a flag checking @requires dependencies first, then evaluate its rules.
fn evaluate_flag_with_reason(
    flag_name: &str,
    context: &Context,
    all_flags: &HashMap<String, Vec<Rule>>,
    metadata: &HashMap<String, FlagMetadata>,
    segments: &Segments,
    env: Option<&str>,
) -> Option<(FlagReturn, &'static str)> {
    // Check @requires prerequisites
    if let Some(meta) = metadata.get(flag_name) {
        for req in &meta.requires {
            match all_flags.get(req.as_str()) {
                None => return None, // required flag doesn't exist
                Some(req_rules) => {
                    match evaluate_rules_with_reason(
                        req_rules,
                        context,
                        Some(req.as_str()),
                        segments,
                        env,
                    ) {
                        Some((FlagReturn::OnOff(true), _)) => {} // prerequisite satisfied
                        _ => return None,                        // prerequisite not met
                    }
                }
            }
        }
    }

    let rules = all_flags.get(flag_name)?;
    evaluate_rules_with_reason(rules, context, Some(flag_name), segments, env)
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

    if !store.flags.contains_key(&key) {
        return (
            StatusCode::NOT_FOUND,
            Json(OFREPEvalError {
                key: key.clone(),
                error_code: "FLAG_NOT_FOUND".to_string(),
                error_details: format!("Flag '{}' was not found", key),
            }),
        )
            .into_response();
    }

    let string_ctx = body
        .context
        .as_ref()
        .map(build_context_from_ofrep)
        .unwrap_or_default();

    let context: Context = string_ctx
        .iter()
        .map(|(k, v)| (k.as_str(), Atom::from(v.as_str())))
        .collect();

    match evaluate_flag_with_reason(
        &key,
        &context,
        &store.flags,
        &store.metadata,
        &store.segments,
        store.env.as_deref(),
    ) {
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
    for key in store.flags.keys() {
        let result = match evaluate_flag_with_reason(
            key,
            &context,
            &store.flags,
            &store.metadata,
            &store.segments,
            store.env.as_deref(),
        ) {
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

fn parse_flags(
    content: &str,
) -> Option<(
    HashMap<String, Vec<Rule>>,
    HashMap<String, FlagMetadata>,
    Segments,
)> {
    let (remainder, parsed) = match parse_flagfile_with_segments(content) {
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
    let mut metadata: HashMap<String, FlagMetadata> = HashMap::new();
    for fv in &parsed.flags {
        for (name, def) in fv.iter() {
            flags.insert(name.to_string(), def.rules.clone());
            metadata.insert(name.to_string(), def.metadata.clone());
        }
    }
    Some((flags, metadata, parsed.segments))
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
            Some((flags, metadata, segments)) => {
                let mut store = state.store.write().await;
                store.flagfile_content = content;
                store.flags = flags;
                store.metadata = metadata;
                store.segments = segments;
                println!("Flagfile reloaded successfully");
            }
            None => {
                // parse_flags already printed the warning
            }
        }
    }
}

pub async fn run_serve(
    flagfile_arg: Option<String>,
    port_arg: Option<u16>,
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

    // Spawn file watcher
    let watcher_state = Arc::clone(&state);
    let watcher_path = PathBuf::from(&flagfile_path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(&flagfile_path));
    tokio::spawn(watch_flagfile(watcher_state, watcher_path));

    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/flagfile", get(handle_flagfile))
        .route("/v1/eval/{flag_name}", get(handle_eval))
        .route("/ofrep/v1/evaluate/flags/{key}", post(handle_ofrep_single))
        .route("/ofrep/v1/evaluate/flags", post(handle_ofrep_bulk))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
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

    axum::serve(listener, app).await.unwrap_or_else(|e| {
        eprintln!("Server error: {}", e);
        process::exit(1);
    });
}
