use std::time::Duration;

use axum::Router;
use axum::http::{HeaderValue, Method};
use axum::routing::get;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::openapi::ApiDoc;
use crate::routes;
use service_kit::middleware;
use service_kit::state::AppState;

/// Maximum request body size (10 MB).
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Build the CORS layer from the `CORS_ALLOWED_ORIGINS` env var.
///
/// Defaults to `http://localhost:5173` (Vite dev server).
/// Multiple origins can be comma-separated: `http://localhost:5173,https://app.example.com`
fn build_cors_layer() -> CorsLayer {
    let origins_str = std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_else(|_| {
        let is_prod = std::env::var("RUST_ENV")
            .map(|v| v == "production")
            .unwrap_or(false);
        if is_prod {
            tracing::warn!("CORS_ALLOWED_ORIGINS not set in production — defaulting to localhost");
        }
        "http://localhost:5173".into()
    });

    let origins: Vec<HeaderValue> = origins_str
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            "content-type".parse().unwrap(),
            "authorization".parse().unwrap(),
            "x-request-id".parse().unwrap(),
        ])
        .max_age(Duration::from_secs(3600))
}

/// Build the complete application router with middleware and SPA fallback.
///
/// Health and metrics endpoints are exempt from rate limiting to prevent
/// Kubernetes probe failures under load.
///
/// Middleware execution order (outermost → innermost):
///
/// ```text
/// CatchPanic → SecurityHeaders → RequestId → RateLimit → Metrics → GlobalTimeout → Trace → CORS → Compression → Handler
/// ```
///
/// Health/metrics routes bypass RateLimit and GlobalTimeout.
pub fn build_router(state: AppState) -> Router {
    // ── Infrastructure routes (exempt from rate limiting + timeout) ───
    let infra = Router::new()
        .route("/health/live", get(routes::health::liveness))
        .route("/health/ready", get(routes::health::readiness))
        .route("/metrics", get(routes::health::metrics_handler));

    // ── API routes (rate-limited + timeout-protected) ────────────────
    let api = Router::new()
        .nest("/api", routes::analytics::router(state.clone()))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .fallback_service(
            ServeDir::new("frontend/dist")
                .not_found_service(ServeFile::new("frontend/dist/index.html")),
        )
        // API-specific layers: timeout + rate limit
        .layer(axum::middleware::from_fn(middleware::global_timeout))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::rate_limit,
        ));

    // ── Merge and apply shared middleware ─────────────────────────────
    Router::new()
        .merge(infra)
        .merge(api)
        .layer(CompressionLayer::new())
        .layer(build_cors_layer())
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn(middleware::metrics_layer))
        .layer(axum::middleware::from_fn(middleware::request_id))
        .layer(axum::middleware::from_fn(middleware::security_headers))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_SIZE))
        .layer(CatchPanicLayer::new())
        .with_state(state)
}
