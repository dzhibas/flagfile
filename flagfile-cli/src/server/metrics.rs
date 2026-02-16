use std::sync::{Arc, OnceLock};
use std::time::Instant;

use axum::extract::MatchedPath;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::state::AppState;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Opts,
    Registry, TextEncoder,
};

/// Global metrics registry
static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// All application metrics
pub struct Metrics {
    // Raft cluster metrics
    pub raft_state: IntGaugeVec,
    pub raft_term: IntGaugeVec,
    pub raft_last_applied: IntGaugeVec,
    pub raft_committed: IntGaugeVec,
    pub raft_peers_connected: IntGaugeVec,
    pub raft_leader_id: IntGaugeVec,
    pub raft_elections: IntCounterVec,
    pub raft_snapshots: IntCounterVec,

    // Flag serving
    pub flags_total: IntGaugeVec,

    // Push metrics
    pub push_total: IntCounterVec,
    pub push_duration: HistogramVec,

    // Eval metrics
    pub eval_total: IntCounterVec,
    pub eval_duration: HistogramVec,
    pub eval_errors: IntCounterVec,

    // SSE metrics
    pub sse_active: IntGaugeVec,
    pub sse_total: IntCounterVec,
    pub sse_events: IntCounterVec,

    // gRPC metrics
    pub grpc_requests: IntCounterVec,
    pub grpc_errors: IntCounterVec,
    pub grpc_latency: HistogramVec,

    // Storage metrics
    pub storage_backend: IntGaugeVec,
    pub storage_size: IntGauge,
    pub storage_write_duration: HistogramVec,

    // HTTP request metrics
    pub http_requests_total: IntCounterVec,
    pub http_request_duration: HistogramVec,
}

static METRICS: OnceLock<Metrics> = OnceLock::new();

impl Metrics {
    fn new(registry: &Registry) -> Self {
        // ── Raft cluster metrics ─────────────────────────────────────
        let raft_state = IntGaugeVec::new(
            Opts::new("ff_raft_state", "Current raft state (0=none, 1=follower, 2=candidate, 3=leader)"),
            &["node_id"],
        )
        .expect("failed to create raft_state metric");

        let raft_term = IntGaugeVec::new(
            Opts::new("ff_raft_term", "Current raft term"),
            &["node_id"],
        )
        .expect("failed to create raft_term metric");

        let raft_last_applied = IntGaugeVec::new(
            Opts::new("ff_raft_last_applied", "Last applied log index"),
            &["node_id"],
        )
        .expect("failed to create raft_last_applied metric");

        let raft_committed = IntGaugeVec::new(
            Opts::new("ff_raft_committed", "Last committed log index"),
            &["node_id"],
        )
        .expect("failed to create raft_committed metric");

        let raft_peers_connected = IntGaugeVec::new(
            Opts::new("ff_raft_peers_connected", "Number of connected raft peers"),
            &["node_id"],
        )
        .expect("failed to create raft_peers_connected metric");

        let raft_leader_id = IntGaugeVec::new(
            Opts::new("ff_raft_leader_id", "Current raft leader node ID"),
            &["node_id"],
        )
        .expect("failed to create raft_leader_id metric");

        let raft_elections = IntCounterVec::new(
            Opts::new("ff_raft_elections_total", "Total number of raft elections"),
            &["node_id"],
        )
        .expect("failed to create raft_elections metric");

        let raft_snapshots = IntCounterVec::new(
            Opts::new("ff_raft_snapshots_total", "Total number of raft snapshots"),
            &["node_id"],
        )
        .expect("failed to create raft_snapshots metric");

        // ── Flag serving metrics ─────────────────────────────────────
        let flags_total = IntGaugeVec::new(
            Opts::new("ff_flags_total", "Total number of flags per namespace"),
            &["namespace"],
        )
        .expect("failed to create flags_total metric");

        // ── Push metrics ─────────────────────────────────────────────
        let push_total = IntCounterVec::new(
            Opts::new("ff_push_total", "Total number of flag pushes"),
            &["namespace", "status"],
        )
        .expect("failed to create push_total metric");

        let push_duration = HistogramVec::new(
            HistogramOpts::new("ff_push_duration_seconds", "Duration of flag push operations")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
            &["namespace"],
        )
        .expect("failed to create push_duration metric");

        // ── Eval metrics ─────────────────────────────────────────────
        let eval_total = IntCounterVec::new(
            Opts::new("ff_eval_total", "Total number of flag evaluations"),
            &["namespace", "flag"],
        )
        .expect("failed to create eval_total metric");

        let eval_duration = HistogramVec::new(
            HistogramOpts::new("ff_eval_duration_seconds", "Duration of flag evaluations")
                .buckets(vec![0.00001, 0.00005, 0.0001, 0.0005, 0.001, 0.005, 0.01]),
            &["namespace"],
        )
        .expect("failed to create eval_duration metric");

        let eval_errors = IntCounterVec::new(
            Opts::new("ff_eval_errors_total", "Total number of flag evaluation errors"),
            &["namespace"],
        )
        .expect("failed to create eval_errors metric");

        // ── SSE metrics ──────────────────────────────────────────────
        let sse_active = IntGaugeVec::new(
            Opts::new("ff_sse_active_connections", "Number of active SSE connections"),
            &["namespace"],
        )
        .expect("failed to create sse_active metric");

        let sse_total = IntCounterVec::new(
            Opts::new("ff_sse_connections_total", "Total number of SSE connections"),
            &["namespace"],
        )
        .expect("failed to create sse_total metric");

        let sse_events = IntCounterVec::new(
            Opts::new("ff_sse_events_total", "Total number of SSE events sent"),
            &["namespace", "type"],
        )
        .expect("failed to create sse_events metric");

        // ── gRPC metrics ─────────────────────────────────────────────
        let grpc_requests = IntCounterVec::new(
            Opts::new("ff_grpc_requests_total", "Total number of gRPC requests"),
            &["peer_id", "method"],
        )
        .expect("failed to create grpc_requests metric");

        let grpc_errors = IntCounterVec::new(
            Opts::new("ff_grpc_errors_total", "Total number of gRPC errors"),
            &["peer_id"],
        )
        .expect("failed to create grpc_errors metric");

        let grpc_latency = HistogramVec::new(
            HistogramOpts::new("ff_grpc_latency_seconds", "gRPC request latency")
                .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
            &["peer_id"],
        )
        .expect("failed to create grpc_latency metric");

        // ── Storage metrics ──────────────────────────────────────────
        let storage_backend = IntGaugeVec::new(
            Opts::new("ff_storage_backend", "Storage backend type (1=active)"),
            &["type"],
        )
        .expect("failed to create storage_backend metric");

        let storage_size = IntGauge::new(
            "ff_storage_size_bytes",
            "Total storage size in bytes",
        )
        .expect("failed to create storage_size metric");

        let storage_write_duration = HistogramVec::new(
            HistogramOpts::new(
                "ff_storage_write_duration_seconds",
                "Duration of storage write operations",
            )
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01]),
            &[],
        )
        .expect("failed to create storage_write_duration metric");

        // ── HTTP request metrics ──────────────────────────────────────
        let http_requests_total = IntCounterVec::new(
            Opts::new("ff_http_requests_total", "Total number of HTTP requests"),
            &["method", "path", "status"],
        )
        .expect("failed to create http_requests_total metric");

        let http_request_duration = HistogramVec::new(
            HistogramOpts::new(
                "ff_http_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .buckets(vec![0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
            &["method", "path"],
        )
        .expect("failed to create http_request_duration metric");

        // Register all metrics with the registry
        registry.register(Box::new(raft_state.clone())).expect("register raft_state");
        registry.register(Box::new(raft_term.clone())).expect("register raft_term");
        registry.register(Box::new(raft_last_applied.clone())).expect("register raft_last_applied");
        registry.register(Box::new(raft_committed.clone())).expect("register raft_committed");
        registry.register(Box::new(raft_peers_connected.clone())).expect("register raft_peers_connected");
        registry.register(Box::new(raft_leader_id.clone())).expect("register raft_leader_id");
        registry.register(Box::new(raft_elections.clone())).expect("register raft_elections");
        registry.register(Box::new(raft_snapshots.clone())).expect("register raft_snapshots");
        registry.register(Box::new(flags_total.clone())).expect("register flags_total");
        registry.register(Box::new(push_total.clone())).expect("register push_total");
        registry.register(Box::new(push_duration.clone())).expect("register push_duration");
        registry.register(Box::new(eval_total.clone())).expect("register eval_total");
        registry.register(Box::new(eval_duration.clone())).expect("register eval_duration");
        registry.register(Box::new(eval_errors.clone())).expect("register eval_errors");
        registry.register(Box::new(sse_active.clone())).expect("register sse_active");
        registry.register(Box::new(sse_total.clone())).expect("register sse_total");
        registry.register(Box::new(sse_events.clone())).expect("register sse_events");
        registry.register(Box::new(grpc_requests.clone())).expect("register grpc_requests");
        registry.register(Box::new(grpc_errors.clone())).expect("register grpc_errors");
        registry.register(Box::new(grpc_latency.clone())).expect("register grpc_latency");
        registry.register(Box::new(storage_backend.clone())).expect("register storage_backend");
        registry.register(Box::new(storage_size.clone())).expect("register storage_size");
        registry.register(Box::new(storage_write_duration.clone())).expect("register storage_write_duration");
        registry.register(Box::new(http_requests_total.clone())).expect("register http_requests_total");
        registry.register(Box::new(http_request_duration.clone())).expect("register http_request_duration");

        Self {
            raft_state,
            raft_term,
            raft_last_applied,
            raft_committed,
            raft_peers_connected,
            raft_leader_id,
            raft_elections,
            raft_snapshots,
            flags_total,
            push_total,
            push_duration,
            eval_total,
            eval_duration,
            eval_errors,
            sse_active,
            sse_total,
            sse_events,
            grpc_requests,
            grpc_errors,
            grpc_latency,
            storage_backend,
            storage_size,
            storage_write_duration,
            http_requests_total,
            http_request_duration,
        }
    }
}

/// Get the global metrics instance, initializing on first call
pub fn metrics() -> &'static Metrics {
    METRICS.get_or_init(|| {
        let registry = REGISTRY.get_or_init(Registry::new);
        Metrics::new(registry)
    })
}

/// Axum handler for GET /metrics — returns Prometheus text format
pub async fn handle_metrics() -> Response {
    // Ensure all metric collectors are registered on first call.
    let _ = metrics();
    let registry = REGISTRY.get_or_init(Registry::new);
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).expect("encode metrics");
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        buffer,
    )
        .into_response()
}

/// Axum handler for GET /readyz
///
/// In cluster mode: ready once a Raft leader is elected.
/// In standalone mode: ready once at least one namespace is loaded.
pub async fn handle_readyz(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Response {
    let namespaces_loaded = state.namespaces.read().await.len();

    if let Some(handle) = state.raft_handle.get() {
        let leader_id = handle.leader_id();
        let raft_state = if handle.is_leader() {
            "leader"
        } else if leader_id > 0 {
            "follower"
        } else {
            "candidate"
        };

        if leader_id == 0 {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(serde_json::json!({
                    "ready": false,
                    "reason": "no leader elected",
                    "raft_state": raft_state,
                })),
            )
                .into_response();
        }

        return (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "ready": true,
                "raft_state": raft_state,
                "leader_id": leader_id,
                "node_id": handle.node_id(),
                "namespaces_loaded": namespaces_loaded,
            })),
        )
            .into_response();
    }

    // Standalone mode (no cluster)
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "ready": true,
            "namespaces_loaded": namespaces_loaded,
        })),
    )
        .into_response()
}

/// Axum middleware that records HTTP request count and duration.
pub async fn track_metrics(request: Request<axum::body::Body>, next: Next) -> Response {
    let method = request.method().to_string();
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());

    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let m = metrics();
    m.http_requests_total
        .with_label_values(&[&method, &path, &status])
        .inc();
    m.http_request_duration
        .with_label_values(&[&method, &path])
        .observe(elapsed);

    response
}

/// Axum handler for GET /health — always returns 200
pub async fn handle_health_check() -> Response {
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({"status": "ok"})),
    )
        .into_response()
}
