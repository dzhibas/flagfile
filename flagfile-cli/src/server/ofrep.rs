use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use flagfile_lib::ast::Atom;
use flagfile_lib::eval::Context;
use flagfile_lib::parse_flagfile::FlagReturn;

use super::routes::evaluate_flag_with_reason;
use super::state::AppState;

#[derive(serde::Deserialize)]
pub struct OFREPEvalRequest {
    pub context: Option<HashMap<String, serde_json::Value>>,
}

#[derive(serde::Serialize)]
pub struct OFREPEvalSuccess {
    key: String,
    reason: String,
    variant: String,
    value: serde_json::Value,
    metadata: serde_json::Value,
}

#[derive(serde::Serialize)]
pub struct OFREPEvalError {
    key: String,
    #[serde(rename = "errorCode")]
    error_code: String,
    #[serde(rename = "errorDetails")]
    error_details: String,
}

#[derive(serde::Serialize)]
pub struct OFREPBulkResponse {
    flags: Vec<serde_json::Value>,
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

pub async fn handle_ofrep_single(
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

pub async fn handle_ofrep_bulk(
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
