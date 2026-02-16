use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::response::sse::{Event, Sse};
use futures::Stream;
use serde::Serialize;
use tokio::sync::{broadcast, RwLock};

use super::metrics::metrics;

/// Event sent when a flagfile is updated
#[derive(Debug, Clone, Serialize)]
pub struct FlagUpdateEvent {
    pub hash: String,
    pub timestamp: String,
    pub flags_count: u64,
}

/// Manages SSE broadcast channels per namespace
pub struct SseBroadcaster {
    channels: RwLock<HashMap<String, broadcast::Sender<FlagUpdateEvent>>>,
    shutdown_tx: broadcast::Sender<()>,
}

impl SseBroadcaster {
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            channels: RwLock::new(HashMap::new()),
            shutdown_tx,
        }
    }

    /// Get or create a broadcast channel for a namespace.
    /// Returns a receiver for subscribing.
    pub async fn subscribe(&self, namespace: &str) -> broadcast::Receiver<FlagUpdateEvent> {
        let mut channels = self.channels.write().await;
        let tx = channels
            .entry(namespace.to_string())
            .or_insert_with(|| broadcast::channel(256).0);
        tx.subscribe()
    }

    /// Subscribe to the shutdown signal.
    pub fn subscribe_shutdown(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Broadcast an event to all subscribers of a namespace.
    pub async fn broadcast(&self, namespace: &str, event: FlagUpdateEvent) {
        let channels = self.channels.read().await;
        if let Some(tx) = channels.get(namespace) {
            let _ = tx.send(event); // ignore error if no subscribers
        }
    }

    /// Signal all SSE clients that the server is shutting down.
    /// Each stream will emit a `server_shutdown` event and close.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

/// SSE handler for a specific namespace.
///
/// Stream format:
/// - On connect: `event: connected\ndata: {"hash":"...","flags_count":N}\n\n`
/// - On update: `event: flag_update\ndata: {"hash":"...","timestamp":"...","flags_count":N}\n\n`
/// - Every 30s: `event: heartbeat\ndata: {}\n\n`
pub async fn create_sse_stream(
    broadcaster: Arc<SseBroadcaster>,
    namespace: String,
    current_hash: Option<String>,
    current_flags_count: Option<u64>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = broadcaster.subscribe(&namespace).await;
    let mut shutdown_rx = broadcaster.subscribe_shutdown();
    let ns = namespace.clone();

    let stream = async_stream::stream! {
        // Track SSE connection
        let m = metrics();
        m.sse_active.with_label_values(&[&ns]).inc();
        m.sse_total.with_label_values(&[&ns]).inc();

        // Yield initial "connected" event with current state
        let connected_data = serde_json::json!({
            "hash": current_hash.unwrap_or_default(),
            "flags_count": current_flags_count.unwrap_or(0),
        });
        yield Ok(Event::default()
            .event("connected")
            .data(connected_data.to_string()));

        // Loop: wait for broadcast updates or send heartbeat every 30s
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(event) => {
                            metrics().sse_events.with_label_values(&[&ns, "flag_update"]).inc();
                            let data = serde_json::json!({
                                "hash": event.hash,
                                "timestamp": event.timestamp,
                                "flags_count": event.flags_count,
                            });
                            yield Ok(Event::default()
                                .event("flag_update")
                                .data(data.to_string()));
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            metrics().sse_events.with_label_values(&[&ns, "lag_warning"]).inc();
                            let data = serde_json::json!({
                                "warning": format!("missed {} events", n),
                            });
                            yield Ok(Event::default()
                                .event("lag_warning")
                                .data(data.to_string()));
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    yield Ok(Event::default()
                        .event("server_shutdown")
                        .data("{\"reason\":\"server restarting\"}".to_string()));
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    metrics().sse_events.with_label_values(&[&ns, "heartbeat"]).inc();
                    yield Ok(Event::default()
                        .event("heartbeat")
                        .data("{}".to_string()));
                }
            }
        }

        // Decrement active connections when stream ends
        metrics().sse_active.with_label_values(&[&ns]).dec();
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
