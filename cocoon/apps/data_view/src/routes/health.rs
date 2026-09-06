use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use service_kit::state::AppState;

/// Liveness probe — confirms the process is running.
///
/// Always returns 200. Use this for Kubernetes `livenessProbe` to detect
/// a hung process (e.g. deadlock). It intentionally does NOT check
/// downstream dependencies so that a transient DB outage does not
/// cause a restart loop.
#[utoipa::path(
    get,
    path = "/health/live",
    tag = "Health",
    responses((status = 200, description = "Process is alive", body = serde_json::Value))
)]
pub async fn liveness() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "alive" }))
}

/// Readiness probe — confirms the service can handle traffic.
///
/// Pings the database and reports connection pool statistics.
/// Returns 503 if the database is unreachable, causing the load balancer
/// or Kubernetes `readinessProbe` to stop sending traffic until recovery.
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "Health",
    responses(
        (status = 200, description = "Service is ready to accept traffic", body = serde_json::Value),
        (status = 503, description = "Service is not ready")
    )
)]
pub async fn readiness(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let start = std::time::Instant::now();
    let s = state.clone();

    let ping_result = tokio::task::spawn_blocking(move || s.ping())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match ping_result {
        Ok(()) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            let pool = state.pool_state();

            Ok(Json(serde_json::json!({
                "status": "ready",
                "checks": {
                    "database": {
                        "status": "up",
                        "latency_ms": latency_ms,
                    },
                    "pool": {
                        "active": pool.connections - pool.idle_connections,
                        "idle": pool.idle_connections,
                        "total": pool.connections,
                    }
                }
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, "Readiness check failed");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

/// Prometheus metrics endpoint.
///
/// Renders all recorded metrics (HTTP RED, DB query stats, pool gauges)
/// in Prometheus exposition format for scraping.
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "Health",
    responses((status = 200, description = "Prometheus metrics in exposition format"))
)]
pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    state.render_metrics()
}
