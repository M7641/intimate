//! hive — a deliberately tiny Axum service whose only job is to make
//! *horizontal scaling* observable.
//!
//! Running one copy of a web server is easy. The interesting engineering
//! starts when you run N identical copies (pods) behind one address and expect
//! the fleet to survive a pod dying, a node draining, and a new version rolling
//! out — all without dropping a request. This binary exposes just enough
//! surface to watch each of those things happen:
//!
//!   GET /            — who am I? returns the pod that served THIS request, so
//!                      repeated calls reveal the Service load-balancing across
//!                      replicas.
//!   GET /healthz     — liveness: "is this process wedged?" Restart me if not 200.
//!   GET /readyz      — readiness: "should traffic come to me right now?" Kept
//!                      out of rotation during warm-up and during shutdown.
//!   GET /work?ms=200 — burn a little CPU, so an HorizontalPodAutoscaler has
//!                      something to react to.
//!
//! The design rule: everything Kubernetes needs to know about this pod, the pod
//! tells the truth about. That is what turns a pile of YAML into a system you
//! can reason about.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use clap::Parser;
use serde::Deserialize;
use serde_json::json;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "hive", about = "A tiny replica-aware Axum service for Kubernetes demos")]
struct Cli {
    /// Address to listen on. In-cluster this is always 0.0.0.0:<port> so the
    /// kubelet and the Service can reach the pod from other nodes.
    #[arg(long, env = "HIVE_ADDR", default_value = "0.0.0.0:3000")]
    addr: String,

    /// Milliseconds to stay "not ready" after boot, simulating a real service
    /// warming a cache or opening a connection pool before it can serve.
    #[arg(long, env = "HIVE_WARMUP_MS", default_value = "0")]
    warmup_ms: u64,
}

/// State shared by every request handler on this pod. `Arc` because Axum clones
/// the state per request across the tokio worker threads.
struct AppState {
    /// The pod's own name, injected by Kubernetes via the Downward API. Without
    /// the Downward API this falls back to the hostname, which in a pod IS the
    /// pod name — but making it explicit documents the dependency.
    pod: String,
    /// The node the pod landed on, also from the Downward API. Seeing two
    /// replicas report different nodes is how you confirm anti-affinity worked.
    node: String,
    /// Requests served by THIS pod since it started. Compare the counter across
    /// pods to see how evenly the Service spreads load.
    served: AtomicU64,
    /// Flipped to false during shutdown so /readyz fails and the Service drains
    /// this pod BEFORE the process actually stops accepting connections.
    ready: AtomicBool,
    started: Instant,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .json()
        .init();

    let cli = Cli::parse();

    let state = Arc::new(AppState {
        pod: env_or("HIVE_POD_NAME", "POD_NAME", "unknown-pod"),
        node: env_or("HIVE_NODE_NAME", "NODE_NAME", "unknown-node"),
        served: AtomicU64::new(0),
        // Not ready until warm-up finishes. The Service will hold traffic back.
        ready: AtomicBool::new(cli.warmup_ms == 0),
        started: Instant::now(),
    });

    if cli.warmup_ms > 0 {
        let state = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(cli.warmup_ms)).await;
            state.ready.store(true, Ordering::SeqCst);
            tracing::info!("warm-up complete, now ready for traffic");
        });
    }

    let app = Router::new()
        .route("/", get(whoami))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/work", get(work))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&cli.addr).await?;
    tracing::info!(pod = %state.pod, node = %state.node, addr = %cli.addr, "hive is up");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await?;

    tracing::info!("shutdown complete");
    Ok(())
}

/// Identify the pod that handled this request. Repeated `curl` calls through the
/// Service should cycle across pod names — that is horizontal scaling working.
async fn whoami(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let n = state.served.fetch_add(1, Ordering::Relaxed) + 1;
    Json(json!({
        "pod": state.pod,
        "node": state.node,
        "served_by_this_pod": n,
        "uptime_secs": state.started.elapsed().as_secs(),
    }))
}

/// Liveness. Answers "is the event loop alive?" — nothing more. It must NOT
/// check dependencies (DB, cache): a failing dependency should not make
/// Kubernetes kill and restart the pod, which would only remove capacity while
/// the dependency is already struggling. Liveness failure means "restart me".
async fn healthz() -> StatusCode {
    StatusCode::OK
}

/// Readiness. Answers "should I receive traffic right now?" This is the gate the
/// Service respects: fail it and the endpoint is pulled from rotation without
/// the pod being killed. We fail it during warm-up and during shutdown.
async fn readyz(State(state): State<Arc<AppState>>) -> StatusCode {
    if state.ready.load(Ordering::SeqCst) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

#[derive(Deserialize)]
struct WorkParams {
    /// How long to keep one CPU busy. This is deliberately CPU-bound (not a
    /// sleep) so an HPA watching CPU utilisation has a real signal to scale on.
    #[serde(default = "default_work_ms")]
    ms: u64,
}
fn default_work_ms() -> u64 {
    200
}

/// Burn CPU for `ms` milliseconds on a blocking worker so we don't starve the
/// async runtime. Hammer this endpoint and watch the HPA add replicas.
async fn work(Query(p): Query<WorkParams>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let ms = p.ms.min(5_000); // guard: never let a caller pin a core forever
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let mut spins: u64 = 0;
        while start.elapsed().as_millis() < ms as u128 {
            spins = spins.wrapping_add(1);
        }
        spins
    })
    .await
    .map(|spins| {
        Json(json!({ "pod": state.pod, "worked_ms": ms, "spins": spins }))
    })
    .unwrap_or_else(|_| Json(json!({ "error": "work task panicked" })))
}

/// Wait for a shutdown signal, then start draining. The ORDER here is the whole
/// trick behind zero-downtime rolling updates:
///
///   1. Kubernetes sends SIGTERM and, concurrently, removes the pod from the
///      Service endpoints — but that removal is eventually-consistent, so some
///      in-flight and just-arriving requests still hit this pod.
///   2. We flip readiness to false so /readyz starts failing immediately,
///      reinforcing the removal.
///   3. We pause briefly to let the endpoint change propagate to every kube-proxy
///      before we stop accepting connections. (In production the same pause also
///      lives in a preStop hook.)
///   4. Only then do we return, which lets Axum's graceful shutdown finish the
///      requests already in flight and close the listener.
async fn shutdown_signal(state: Arc<AppState>) {
    let sigterm = async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            signal(SignalKind::terminate())
                .expect("install SIGTERM handler")
                .recv()
                .await;
        }
        #[cfg(not(unix))]
        std::future::pending::<()>().await;
    };
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("install Ctrl-C handler");
    };

    tokio::select! {
        _ = sigterm => tracing::info!("SIGTERM received, draining"),
        _ = ctrl_c => tracing::info!("Ctrl-C received, draining"),
    }

    state.ready.store(false, Ordering::SeqCst);
    // Let the endpoint removal reach every node before we stop accepting.
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
}

/// Read the first non-empty of two env var names, else a default. Lets the chart
/// use `HIVE_POD_NAME` explicitly while still honouring a bare `POD_NAME`.
fn env_or(primary: &str, fallback: &str, default: &str) -> String {
    std::env::var(primary)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var(fallback).ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| default.to_string())
}
