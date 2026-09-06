use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tracing::Instrument;
use uuid::Uuid;

use crate::state::AppState;

// ── Request ID ──────────────────────────────────────────────────────────

/// Newtype so handlers can extract the request ID from extensions.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RequestId(pub String);

/// Assigns a unique request ID to each request and wraps the downstream
/// processing in a tracing span carrying that ID.
///
/// - Reuses an incoming `x-request-id` header when present (e.g. from an API gateway).
/// - Otherwise generates a UUID v4.
/// - The ID is returned in the `x-request-id` response header for client correlation.
pub async fn request_id(mut req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    req.extensions_mut().insert(RequestId(id.clone()));

    let span = tracing::info_span!("request", request_id = %id);

    async move {
        let mut res = next.run(req).await;
        if let Ok(val) = id.parse() {
            res.headers_mut().insert("x-request-id", val);
        }
        res
    }
    .instrument(span)
    .await
}

// ── RED Metrics ─────────────────────────────────────────────────────────

/// Records RED (Rate, Errors, Duration) metrics for every HTTP request.
///
/// Uses the matched route pattern (e.g. `/api/data_view/columns/{table_name}`)
/// to keep metric cardinality bounded. Falls back to `"unmatched"` for
/// requests that hit the SPA fallback or unknown paths.
pub async fn metrics_layer(
    matched_path: Option<MatchedPath>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().to_string();
    let path = matched_path
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string());

    let start = Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    metrics::counter!(
        "http_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status
    )
    .increment(1);

    metrics::histogram!(
        "http_request_duration_seconds",
        "method" => method,
        "path" => path
    )
    .record(duration);

    response
}

// ── Timeouts ────────────────────────────────────────────────────────────

/// Global request timeout (default 60s, configurable via `GLOBAL_TIMEOUT_SECS`).
///
/// Returns 504 Gateway Timeout with a JSON body if the handler doesn't
/// complete in time. This is the *outermost* HTTP timeout, so it caps every
/// route — including the heavy warehouse routes. Keep it >= `HEAVY_TIMEOUT_SECS`
/// and the DB `STATEMENT_TIMEOUT_SECS`, or it will cut them short.
pub async fn global_timeout(req: Request, next: Next) -> Response {
    let secs = std::env::var("GLOBAL_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60u64);

    match tokio::time::timeout(Duration::from_secs(secs), next.run(req)).await {
        Ok(response) => response,
        Err(_) => {
            metrics::counter!("http_timeouts_total").increment(1);
            tracing::warn!(timeout_secs = secs, "Request timed out");
            (
                StatusCode::GATEWAY_TIMEOUT,
                axum::Json(serde_json::json!({"detail": "Request timed out"})),
            )
                .into_response()
        }
    }
}

/// Timeout for heavy endpoints (default 60s, configurable via `HEAVY_TIMEOUT_SECS`).
///
/// Note this is nested *inside* [`global_timeout`], so the effective limit is
/// `min(GLOBAL_TIMEOUT_SECS, HEAVY_TIMEOUT_SECS)`. Both default to 60s so the two
/// agree out of the box; raise `GLOBAL_TIMEOUT_SECS` too if you set this higher.
pub async fn heavy_route_timeout(req: Request, next: Next) -> Response {
    let secs = std::env::var("HEAVY_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60u64);

    match tokio::time::timeout(Duration::from_secs(secs), next.run(req)).await {
        Ok(response) => response,
        Err(_) => {
            metrics::counter!("http_timeouts_total").increment(1);
            tracing::warn!(timeout_secs = secs, "Heavy request timed out");
            (
                StatusCode::GATEWAY_TIMEOUT,
                axum::Json(serde_json::json!({"detail": "Request timed out"})),
            )
                .into_response()
        }
    }
}

// ── Rate Limiting ───────────────────────────────────────────────────────

/// Global rate limit middleware. Returns 429 with `Retry-After` header when exceeded.
pub async fn rate_limit(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if !state.rate_limiter().try_acquire() {
        metrics::counter!("http_rate_limited_total").increment(1);
        tracing::warn!("Global rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "1")],
            axum::Json(serde_json::json!({"detail": "Too many requests"})),
        )
            .into_response();
    }
    next.run(req).await
}

/// Rate limit for heavy/expensive endpoints. Lower threshold than global.
pub async fn heavy_rate_limit(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if !state.heavy_rate_limiter().try_acquire() {
        metrics::counter!("http_rate_limited_total").increment(1);
        tracing::warn!("Heavy endpoint rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "1")],
            axum::Json(serde_json::json!({"detail": "Too many requests — heavy endpoint limit"})),
        )
            .into_response();
    }
    next.run(req).await
}

// ── Security Headers ────────────────────────────────────────────────────

/// Appends standard security headers to every response.
///
/// Note: `x-frame-options` is intentionally NOT set. That header only accepts
/// `DENY`/`SAMEORIGIN` (its `ALLOW-FROM` form is obsolete and ignored by modern
/// browsers), so there is no "allow any origin" value — its mere presence would
/// block cross-origin framing. Omitting it lets responses be embedded in an
/// iframe from any origin. Trade-off: no built-in clickjacking protection; add a
/// `Content-Security-Policy: frame-ancestors <list>` header if framing must be
/// restricted later.
pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("x-xss-protection", "0".parse().unwrap());
    headers.insert(
        "referrer-policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );
    res
}
