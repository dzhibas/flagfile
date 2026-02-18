use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::StreamExt;
use sha1::Digest;

use super::metrics::metrics;
use super::sse::{FlagUpdateEvent, SseBroadcaster};
use super::state::{AppState, ParsedNamespace};
use super::store::ROOT_NAMESPACE;
use super::watch::parse_flags;

// ── Fetch and update ────────────────────────────────────────

/// Fetch the flagfile from upstream, parse it, and update local state.
/// Returns `true` on success, `false` on failure.
pub async fn fetch_and_update(
    flagfile_url: &str,
    token: Option<&str>,
    state: Arc<AppState>,
    broadcaster: Arc<SseBroadcaster>,
) -> bool {
    let client = reqwest::Client::new();
    let mut req = client.get(flagfile_url);
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }

    let response = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Sidecar: fetch error: {}", e);
            return false;
        }
    };

    if !response.status().is_success() {
        eprintln!(
            "Sidecar: upstream returned {}",
            response.status()
        );
        return false;
    }

    let content = match response.text().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Sidecar: failed to read response body: {}", e);
            return false;
        }
    };

    let (flags, metadata, segments) = match parse_flags(&content) {
        Some(result) => result,
        None => {
            eprintln!("Sidecar: failed to parse upstream flagfile");
            return false;
        }
    };

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
            env: None,
        },
    );
    drop(namespaces);

    broadcaster
        .broadcast(
            ROOT_NAMESPACE,
            FlagUpdateEvent {
                hash,
                timestamp: chrono::Utc::now().to_rfc3339(),
                flags_count,
            },
        )
        .await;

    metrics()
        .flags_total
        .with_label_values(&[ROOT_NAMESPACE])
        .set(flags_count as i64);

    println!("Sidecar: synced {} flags from upstream", flags_count);
    true
}

// ── Upstream SSE listener ───────────────────────────────────

/// Background task that connects to the upstream SSE endpoint and
/// re-fetches the flagfile whenever a relevant event is received.
/// Reconnects with exponential backoff on failure.
pub async fn upstream_sse_listener(
    events_url: String,
    flagfile_url: String,
    token: Option<String>,
    state: Arc<AppState>,
    broadcaster: Arc<SseBroadcaster>,
) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);

    loop {
        let client = reqwest::Client::new();
        let mut req = client
            .get(&events_url)
            .header("Accept", "text/event-stream");
        if let Some(ref t) = token {
            req = req.header("Authorization", format!("Bearer {}", t));
        }

        match req.send().await {
            Ok(response) if response.status().is_success() => {
                backoff = Duration::from_secs(1); // reset on success
                println!("Sidecar: connected to upstream SSE at {}", events_url);

                let mut stream = response.bytes_stream();
                let mut buffer = String::new();
                let mut current_event = String::new();

                while let Some(chunk) = stream.next().await {
                    let chunk = match chunk {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Sidecar: SSE stream error: {}", e);
                            break;
                        }
                    };

                    let text = match std::str::from_utf8(&chunk) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };

                    buffer.push_str(text);

                    // Process complete SSE lines
                    while let Some(newline_pos) = buffer.find('\n') {
                        let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                        buffer = buffer[newline_pos + 1..].to_string();

                        if line.is_empty() {
                            // Empty line = end of event
                            if !current_event.is_empty() {
                                let should_fetch = current_event == "connected"
                                    || current_event == "flag_update";
                                let is_shutdown = current_event == "server_shutdown";

                                if should_fetch || is_shutdown {
                                    fetch_and_update(
                                        &flagfile_url,
                                        token.as_deref(),
                                        Arc::clone(&state),
                                        Arc::clone(&broadcaster),
                                    )
                                    .await;
                                }

                                if is_shutdown {
                                    eprintln!(
                                        "Sidecar: upstream sent server_shutdown, reconnecting..."
                                    );
                                    break;
                                }

                                current_event.clear();
                            }
                        } else if let Some(event_type) = line.strip_prefix("event: ") {
                            current_event = event_type.to_string();
                        }
                        // Ignore "data:", comments (":"), and other fields
                    }
                }

                eprintln!("Sidecar: SSE stream ended, reconnecting...");
            }
            Ok(response) => {
                eprintln!(
                    "Sidecar: upstream SSE returned {}, retrying in {:?}",
                    response.status(),
                    backoff
                );
            }
            Err(e) => {
                eprintln!(
                    "Sidecar: SSE connection failed: {}, retrying in {:?}",
                    e, backoff
                );
            }
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(max_backoff);
    }
}

// ── Read-only PUT handler ───────────────────────────────────

pub async fn handle_put_readonly() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(serde_json::json!({"error": "sidecar is read-only"})),
    )
        .into_response()
}

// ── Sidecar readiness probe ─────────────────────────────────

pub async fn handle_sidecar_readyz(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Response {
    let namespaces = state.namespaces.read().await;
    match namespaces.get(ROOT_NAMESPACE) {
        Some(ns) if !ns.flags.is_empty() => (
            StatusCode::OK,
            Json(serde_json::json!({
                "ready": true,
                "flags_loaded": ns.flags.len(),
            })),
        )
            .into_response(),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "ready": false,
                "reason": "no flags loaded from upstream yet",
            })),
        )
            .into_response(),
    }
}
