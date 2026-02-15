use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use flagfile_lib::ast::{Atom, FlagMetadata};
use flagfile_lib::eval::{eval_with_segments, Context, Segments};
use flagfile_lib::parse_flagfile::{FlagReturn, Rule};

use super::state::AppState;

pub async fn handle_health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let store = state.store.read().await;
    Json(serde_json::json!({
        "status": "ok",
        "flags_loaded": store.flags.len()
    }))
}

pub async fn handle_flagfile(State(state): State<Arc<AppState>>) -> Response {
    let store = state.store.read().await;
    (
        StatusCode::OK,
        [("content-type", "text/plain")],
        store.flagfile_content.clone(),
    )
        .into_response()
}

pub async fn handle_eval(
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

/// Evaluate rules and return the result along with a reason string.
/// Handles all rule types including `EnvRule`.
pub fn evaluate_rules_with_reason(
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
pub fn evaluate_flag_with_reason(
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
