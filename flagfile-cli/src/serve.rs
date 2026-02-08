use std::collections::HashMap;
use std::process;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use flagfile_lib::ast::Atom;
use flagfile_lib::eval::Context;
use flagfile_lib::parse_flagfile::{parse_flagfile, FlagReturn, Rule};

use crate::evaluate_flag;

#[derive(serde::Deserialize, Default)]
struct ServeConfig {
    port: Option<u16>,
    flagfile: Option<String>,
}

struct AppState {
    flagfile_content: String,
    flags: HashMap<String, Vec<Rule>>,
}

async fn handle_flagfile(State(state): State<Arc<AppState>>) -> Response {
    (
        StatusCode::OK,
        [("content-type", "text/plain")],
        state.flagfile_content.clone(),
    )
        .into_response()
}

async fn handle_eval(
    State(state): State<Arc<AppState>>,
    Path(flag_name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let plain = params.get("ff_output").map(|v| v == "plain").unwrap_or(false);

    let Some(rules) = state.flags.get(&flag_name) else {
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
        None => {
            if plain {
                return (StatusCode::UNPROCESSABLE_ENTITY, "no rule matched")
                    .into_response();
            }
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                axum::Json(
                    serde_json::json!({"error": "no rule matched", "flag": flag_name}),
                ),
            )
                .into_response()
        }
    }
}

pub async fn run_serve(
    flagfile_arg: Option<String>,
    port_arg: Option<u16>,
    config_path: &str,
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

    // Read and parse flagfile
    let flagfile_content = match std::fs::read_to_string(&flagfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            process::exit(1);
        }
    };

    let (remainder, flag_values) = match parse_flagfile(&flagfile_content) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Parsing failed: {}", e);
            process::exit(1);
        }
    };

    if !remainder.trim().is_empty() {
        eprintln!(
            "Parsing failed: unexpected content near: {}",
            remainder.trim().lines().next().unwrap_or("")
        );
        process::exit(1);
    }

    let mut flags: HashMap<String, Vec<Rule>> = HashMap::new();
    for fv in &flag_values {
        for (name, rules) in fv.iter() {
            flags.insert(name.to_string(), rules.clone());
        }
    }

    let state = Arc::new(AppState {
        flagfile_content,
        flags,
    });

    let app = Router::new()
        .route("/flagfile", get(handle_flagfile))
        .route("/eval/{flag_name}", get(handle_eval))
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
