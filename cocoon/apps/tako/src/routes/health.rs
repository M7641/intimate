use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sysinfo::{ProcessesToUpdate, System};
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InstanceHealth {
    status: String,
    cpu: CpuInfo,
    memory: MemoryInfo,
    uptime: u64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CpuInfo {
    usage_percent: f32,
    num_cpus: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MemoryInfo {
    total_mb: u64,
    used_mb: u64,
    available_mb: u64,
    usage_percent: f32,
}

/// Liveness: the process is up and serving. Says nothing about dependencies —
/// an orchestrator uses this only to decide whether to restart the process.
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses((status = 200, description = "Process is alive", body = serde_json::Value))
)]
pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

/// Prometheus metrics in exposition format, for scraping. The recorder is fed by
/// `service_kit::middleware::metrics_layer` (request rate / errors / duration).
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    state.metrics_handle.render()
}

/// Readiness: can the service actually do its job right now? Pings the warehouse
/// pool — the critical dependency — and reports `503` if it is unreachable, so an
/// orchestrator stops routing traffic here until the backend recovers.
///
/// The ping is a blocking driver call, so it runs off the async runtime.
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "Health",
    responses(
        (status = 200, description = "Backend reachable; ready for traffic", body = serde_json::Value),
        (status = 503, description = "Backend unreachable; not ready", body = serde_json::Value)
    )
)]
pub async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    let pool = state.db.clone();
    match tokio::task::spawn_blocking(move || pool.ping()).await {
        Ok(Ok(())) => (StatusCode::OK, Json(json!({ "status": "ready" }))),
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "Readiness check failed: database ping error");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "not_ready", "check": "database" })),
            )
        }
        Err(e) => {
            tracing::warn!(error = %e, "Readiness check task panicked");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "not_ready", "check": "database" })),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/instance_health",
    tag = "Health",
    responses((status = 200, description = "Host CPU, memory and uptime", body = InstanceHealth))
)]
pub async fn instance_health() -> impl IntoResponse {
    // Create system instance without loading all processes upfront (vs new_all())
    let mut sys = System::new();
    let pid = sysinfo::get_current_pid().expect("current process always has a PID");

    // Per-process CPU usage is derived from the delta between two refreshes:
    // a single sample has nothing to diff against and always reports 0%. So we
    // refresh this process twice, spaced by the platform's minimum interval.
    // The wait uses tokio so the async runtime thread stays free meanwhile.
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

    let cpu_usage = sys.process(pid).map(|proc| proc.cpu_usage()).unwrap_or(0.0);

    // Memory and CPU topology are point-in-time readings; one refresh each.
    sys.refresh_memory();
    sys.refresh_cpu_all();

    // Get memory info
    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let available_memory = sys.available_memory();
    let memory_percent = (used_memory as f32 / total_memory as f32) * 100.0;

    // Convert bytes to MB
    let total_mb = total_memory / 1024 / 1024;
    let used_mb = used_memory / 1024 / 1024;
    let available_mb = available_memory / 1024 / 1024;

    let health = InstanceHealth {
        status: "healthy".to_string(),
        cpu: CpuInfo {
            usage_percent: cpu_usage,
            num_cpus: sys.cpus().len(),
        },
        memory: MemoryInfo {
            total_mb,
            used_mb,
            available_mb,
            usage_percent: memory_percent,
        },
        uptime: System::uptime(),
    };

    (StatusCode::OK, Json(health))
}
