use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use flagfile_lib::ast::{Atom, FlagMetadata};
use flagfile_lib::eval::{eval_with_segments, Context, Segments};
use flagfile_lib::parse_flagfile::{parse_flagfile_with_segments, FlagReturn, Rule};
use sha1::{Digest, Sha1};

use super::metrics::metrics;

use super::auth::{check_token, extract_bearer_token, forbidden, unauthorized, TokenPermission};
use super::state::{AppState, ParsedNamespace};
use super::store::{Meta, ROOT_NAMESPACE};
use super::sse::{create_sse_stream, FlagUpdateEvent};

// ── Helper: extract bearer token from headers ────────────────

fn get_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(extract_bearer_token)
        .map(|s| s.to_string())
}

// ── Health ───────────────────────────────────────────────────

pub async fn handle_health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let ns = state.namespaces.read().await;
    let total_flags: usize = ns.values().map(|n| n.flags.len()).sum();
    Json(serde_json::json!({
        "status": "ok",
        "flags_loaded": total_flags
    }))
}

// ── GET /flagfile or /ns/{ns}/flagfile ───────────────────────

pub async fn handle_flagfile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ns_param: Option<Path<String>>,
) -> Response {
    let ns_key = AppState::resolve_namespace(ns_param.as_ref().map(|p| p.0.as_str()));
    let ns_config = match state.namespace_config(ns_key) {
        Some(c) => c,
        None => return forbidden(),
    };
    let token = get_token(&headers);

    if !check_token(&ns_config, token.as_deref(), TokenPermission::Read) {
        return unauthorized();
    }

    let namespaces = state.namespaces.read().await;
    match namespaces.get(ns_key) {
        Some(ns) => (
            StatusCode::OK,
            [("content-type", "text/plain")],
            ns.flagfile_content.clone(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "namespace not found").into_response(),
    }
}

// ── GET /flagfile/hash or /ns/{ns}/flagfile/hash ─────────────

pub async fn handle_flagfile_hash(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ns_param: Option<Path<String>>,
) -> Response {
    let ns_key = AppState::resolve_namespace(ns_param.as_ref().map(|p| p.0.as_str()));
    let ns_config = match state.namespace_config(ns_key) {
        Some(c) => c,
        None => return forbidden(),
    };
    let token = get_token(&headers);

    if !check_token(&ns_config, token.as_deref(), TokenPermission::Read) {
        return unauthorized();
    }

    let namespaces = state.namespaces.read().await;
    match namespaces.get(ns_key) {
        Some(ns) => {
            let mut hasher = Sha1::new();
            hasher.update(ns.flagfile_content.as_bytes());
            let hash = format!("{:x}", hasher.finalize());
            (StatusCode::OK, [("content-type", "text/plain")], hash).into_response()
        }
        None => (StatusCode::NOT_FOUND, "namespace not found").into_response(),
    }
}

// ── PUT /flagfile or /ns/{ns}/flagfile ───────────────────────

pub async fn handle_put_flagfile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ns_param: Option<Path<String>>,
    body: String,
) -> Response {
    let start = Instant::now();
    let ns_key = AppState::resolve_namespace(ns_param.as_ref().map(|p| p.0.as_str()));
    let ns_config = match state.namespace_config(ns_key) {
        Some(c) => c,
        None => return forbidden(),
    };
    let token = get_token(&headers);

    if !check_token(&ns_config, token.as_deref(), TokenPermission::Write) {
        return unauthorized();
    }

    // Validate syntax
    let body_for_parse = body.clone();
    let (remainder, parsed) = match parse_flagfile_with_segments(&body_for_parse) {
        Ok(result) => result,
        Err(e) => {
            metrics().push_total.with_label_values(&[ns_key, "error"]).inc();
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": format!("parse error: {}", e)})),
            )
                .into_response();
        }
    };

    if !remainder.trim().is_empty() {
        metrics().push_total.with_label_values(&[ns_key, "error"]).inc();
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "error": format!("unexpected content near: {}",
                    remainder.trim().lines().next().unwrap_or(""))
            })),
        )
            .into_response();
    }

    // Build parsed flags
    let mut flags: HashMap<String, Vec<Rule>> = HashMap::new();
    let mut metadata_map: HashMap<String, FlagMetadata> = HashMap::new();
    for fv in &parsed.flags {
        for (name, def) in fv {
            flags.insert(name.to_string(), def.rules.clone());
            metadata_map.insert(name.to_string(), def.metadata.clone());
        }
    }
    let flags_count = flags.len() as u64;

    // Compute hash
    let mut hasher = Sha1::new();
    hasher.update(body.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    // ── Raft-aware write path ────────────────────────────────
    if let Some(handle) = state.raft_handle.get() {
        if handle.is_leader() {
            // Leader: propose via Raft consensus — the state machine will
            // handle the store write, in-memory update, and SSE broadcast.
            let meta = Meta {
                hash: hash.clone(),
                pushed_at: chrono::Utc::now().to_rfc3339(),
                flags_count,
            };
            let cmd = super::raft::RaftCommand::PutFlagfile {
                namespace: ns_key.to_string(),
                content: body.into_bytes(),
                meta,
            };
            return match handle.propose(cmd).await {
                Ok(()) => {
                    let m = metrics();
                    m.push_total.with_label_values(&[ns_key, "ok"]).inc();
                    m.push_duration.with_label_values(&[ns_key]).observe(start.elapsed().as_secs_f64());
                    m.flags_total.with_label_values(&[ns_key]).set(flags_count as i64);
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "status": "ok",
                            "flags_count": flags_count,
                            "hash": hash,
                        })),
                    )
                        .into_response()
                }
                Err(e) => {
                    metrics().push_total.with_label_values(&[ns_key, "error"]).inc();
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("raft propose: {}", e)})),
                    )
                        .into_response()
                }
            };
        } else {
            // Follower: forward the write to the current leader.
            let leader_id = handle.leader_id();
            if leader_id == 0 {
                metrics().push_total.with_label_values(&[ns_key, "error"]).inc();
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({"error": "no leader elected yet"})),
                )
                    .into_response();
            }
            if let Some(transport) = state.raft_transport.get() {
                let token = get_token(&headers).unwrap_or_default();
                return match transport
                    .forward_write(leader_id, ns_key, body.as_bytes(), &token)
                    .await
                {
                    Ok(resp) if resp.success => {
                        let m = metrics();
                        m.push_total.with_label_values(&[ns_key, "ok"]).inc();
                        m.push_duration.with_label_values(&[ns_key]).observe(start.elapsed().as_secs_f64());
                        m.flags_total.with_label_values(&[ns_key]).set(resp.flags_count as i64);
                        (
                            StatusCode::OK,
                            Json(serde_json::json!({
                                "status": "ok",
                                "flags_count": resp.flags_count,
                                "hash": resp.hash,
                            })),
                        )
                            .into_response()
                    }
                    Ok(resp) => {
                        metrics().push_total.with_label_values(&[ns_key, "error"]).inc();
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": resp.error})),
                        )
                            .into_response()
                    }
                    Err(e) => {
                        metrics().push_total.with_label_values(&[ns_key, "error"]).inc();
                        (
                            StatusCode::BAD_GATEWAY,
                            Json(serde_json::json!({
                                "error": format!("forward to leader failed: {}", e)
                            })),
                        )
                            .into_response()
                    }
                };
            }
        }
    }

    // ── Direct write path (no cluster) ───────────────────────

    // Get env from an existing namespace entry (or None)
    let env = {
        let ns = state.namespaces.read().await;
        ns.get(ns_key).and_then(|n| n.env.clone())
    };

    // Write to persistent store if available
    if let Some(ref store) = state.persistent_store {
        let meta = Meta {
            hash: hash.clone(),
            pushed_at: chrono::Utc::now().to_rfc3339(),
            flags_count,
        };
        if let Err(e) = store.put_flagfile(ns_key, body.as_bytes(), &meta).await {
            metrics().push_total.with_label_values(&[ns_key, "error"]).inc();
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("storage error: {}", e)})),
            )
                .into_response();
        }
    }

    // Update in-memory parsed state
    {
        let mut namespaces = state.namespaces.write().await;
        namespaces.insert(
            ns_key.to_string(),
            ParsedNamespace {
                flagfile_content: body,
                flags,
                metadata: metadata_map,
                segments: parsed.segments,
                env,
            },
        );
    }

    // Broadcast SSE update
    state
        .broadcaster
        .broadcast(
            ns_key,
            FlagUpdateEvent {
                hash: hash.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                flags_count,
            },
        )
        .await;

    let m = metrics();
    m.push_total.with_label_values(&[ns_key, "ok"]).inc();
    m.push_duration.with_label_values(&[ns_key]).observe(start.elapsed().as_secs_f64());
    m.flags_total.with_label_values(&[ns_key]).set(flags_count as i64);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "flags_count": flags_count,
            "hash": hash,
        })),
    )
        .into_response()
}

// ── GET /events or /ns/{ns}/events ───────────────────────────

pub async fn handle_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ns_param: Option<Path<String>>,
) -> Response {
    let ns_key = AppState::resolve_namespace(ns_param.as_ref().map(|p| p.0.as_str()));
    let ns_config = match state.namespace_config(ns_key) {
        Some(c) => c,
        None => return forbidden(),
    };
    let token = get_token(&headers);

    if !check_token(&ns_config, token.as_deref(), TokenPermission::Read) {
        return unauthorized();
    }

    let (current_hash, current_count) = {
        let ns = state.namespaces.read().await;
        match ns.get(ns_key) {
            Some(n) => {
                let mut hasher = Sha1::new();
                hasher.update(n.flagfile_content.as_bytes());
                let hash = format!("{:x}", hasher.finalize());
                (Some(hash), Some(n.flags.len() as u64))
            }
            None => (None, None),
        }
    };

    create_sse_stream(
        Arc::clone(&state.broadcaster),
        ns_key.to_string(),
        current_hash,
        current_count,
    )
    .await
    .into_response()
}

// ── GET /v1/eval/{flag} or /ns/{ns}/v1/eval/{flag} ──────────

pub async fn handle_eval(
    State(state): State<Arc<AppState>>,
    Path(params): Path<EvalParams>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let start = Instant::now();
    let ns_key = params
        .namespace
        .as_deref()
        .unwrap_or(ROOT_NAMESPACE);
    let flag_name = &params.flag_name;

    let ns_config = match state.namespace_config(ns_key) {
        Some(c) => c,
        None => return forbidden(),
    };
    let token = get_token(&headers);
    if state.multi_tenant && !check_token(&ns_config, token.as_deref(), TokenPermission::Read) {
        return unauthorized();
    }

    let namespaces = state.namespaces.read().await;
    let ns = match namespaces.get(ns_key) {
        Some(n) => n,
        None => {
            metrics().eval_errors.with_label_values(&[ns_key]).inc();
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "namespace not found"})),
            )
                .into_response();
        }
    };

    let plain = query
        .get("ff_output")
        .map(|v| v == "plain")
        .unwrap_or(false);

    if !ns.flags.contains_key(flag_name.as_str()) {
        let m = metrics();
        m.eval_total.with_label_values(&[ns_key, flag_name]).inc();
        m.eval_errors.with_label_values(&[ns_key]).inc();
        m.eval_duration.with_label_values(&[ns_key]).observe(start.elapsed().as_secs_f64());
        if plain {
            return (StatusCode::NOT_FOUND, "flag not found").into_response();
        }
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "flag not found", "flag": flag_name})),
        )
            .into_response();
    }

    let context: Context = query
        .iter()
        .filter(|(k, _)| k.as_str() != "ff_output")
        .map(|(k, v)| (k.as_str(), Atom::from(v.as_str())))
        .collect();

    let result = evaluate_flag_with_reason(
        flag_name,
        &context,
        &ns.flags,
        &ns.metadata,
        &ns.segments,
        ns.env.as_deref(),
    );

    let m = metrics();
    m.eval_total.with_label_values(&[ns_key, flag_name]).inc();
    m.eval_duration.with_label_values(&[ns_key]).observe(start.elapsed().as_secs_f64());

    match result {
        Some((val, _reason)) => format_flag_response(flag_name, &val, plain),
        None => {
            m.eval_errors.with_label_values(&[ns_key]).inc();
            if plain {
                (StatusCode::UNPROCESSABLE_ENTITY, "no rule matched").into_response()
            } else {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(serde_json::json!({"error": "no rule matched", "flag": flag_name})),
                )
                    .into_response()
            }
        }
    }
}

/// Path parameters for eval endpoint, supporting both root and namespaced routes.
#[derive(serde::Deserialize)]
pub struct EvalParams {
    pub flag_name: String,
    #[serde(default)]
    pub namespace: Option<String>,
}

fn format_flag_response(flag_name: &str, val: &FlagReturn, plain: bool) -> Response {
    match val {
        FlagReturn::OnOff(v) => {
            if plain {
                return (StatusCode::OK, v.to_string()).into_response();
            }
            (StatusCode::OK, Json(serde_json::json!({"flag": flag_name, "value": v}))).into_response()
        }
        FlagReturn::Json(v) => {
            if plain {
                return (StatusCode::OK, v.to_string()).into_response();
            }
            (StatusCode::OK, Json(serde_json::json!({"flag": flag_name, "value": v}))).into_response()
        }
        FlagReturn::Integer(v) => {
            if plain {
                return (StatusCode::OK, v.to_string()).into_response();
            }
            (StatusCode::OK, Json(serde_json::json!({"flag": flag_name, "value": v}))).into_response()
        }
        FlagReturn::Str(v) => {
            if plain {
                return (StatusCode::OK, v.clone()).into_response();
            }
            (StatusCode::OK, Json(serde_json::json!({"flag": flag_name, "value": v}))).into_response()
        }
    }
}

// ── Evaluation helpers ──────────────────────────────────────

/// Evaluate rules and return the result along with a reason string.
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

/// Evaluate a flag checking @requires dependencies first.
pub fn evaluate_flag_with_reason(
    flag_name: &str,
    context: &Context,
    all_flags: &HashMap<String, Vec<Rule>>,
    metadata: &HashMap<String, FlagMetadata>,
    segments: &Segments,
    env: Option<&str>,
) -> Option<(FlagReturn, &'static str)> {
    if let Some(meta) = metadata.get(flag_name) {
        for req in &meta.requires {
            match all_flags.get(req.as_str()) {
                None => return None,
                Some(req_rules) => {
                    match evaluate_rules_with_reason(
                        req_rules,
                        context,
                        Some(req.as_str()),
                        segments,
                        env,
                    ) {
                        Some((FlagReturn::OnOff(true), _)) => {}
                        _ => return None,
                    }
                }
            }
        }
    }

    let rules = all_flags.get(flag_name)?;
    evaluate_rules_with_reason(rules, context, Some(flag_name), segments, env)
}
