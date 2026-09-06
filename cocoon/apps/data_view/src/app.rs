use std::path::PathBuf;
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::openapi::ApiDoc;
use crate::routes;
use service_kit::middleware;
use service_kit::state::AppState;

/// Maximum request body size (10 MB).
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Locate the built frontend bundle (the directory holding `index.html`).
///
/// `ServeDir` resolves paths against the process's working directory, which
/// differs across our run modes, so we probe a few candidates in priority order
/// and return the first that actually contains a bundle:
///
/// 1. `DATA_VIEW_STATIC_DIR` — explicit override, wins if set.
/// 2. `frontend/dist` relative to the CWD — the Docker image (`WORKDIR /app`)
///    and a dev run started from the app crate.
/// 3. `<crate>/frontend/dist` baked at compile time — a dev run started from
///    anywhere else in the workspace (e.g. `cargo run` at the repo root).
///
/// Returns `None` when no bundle is found (API-only dev, where the UI is served
/// by Vite instead).
pub fn spa_bundle_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var("DATA_VIEW_STATIC_DIR")
            .ok()
            .map(PathBuf::from),
        Some(PathBuf::from("frontend/dist")),
        Some(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/frontend/dist"
        ))),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|dir| dir.join("index.html").is_file())
}

/// Serve the built SPA: real files from the bundle directory, and `index.html`
/// (with a `200`) for any unmatched path so client-side routes like
/// `/data-explorer` resolve on a direct visit or refresh instead of 404-ing.
///
/// The index is read once at startup and held in memory; if no bundle is found
/// the fallback returns `404` (API-only dev — the UI lives on the Vite server).
fn spa_service() -> ServeDir<axum::routing::MethodRouter> {
    let dir = spa_bundle_dir().unwrap_or_else(|| PathBuf::from("frontend/dist"));
    let index_html = std::fs::read_to_string(dir.join("index.html")).ok();

    let index_fallback = get(move || {
        let index_html = index_html.clone();
        async move {
            match index_html {
                Some(html) => Html(html).into_response(),
                None => (StatusCode::NOT_FOUND, "Frontend bundle not found").into_response(),
            }
        }
    });

    // `fallback` (not `not_found_service`) serves the index without overriding
    // the status, so client routes resolve with `200` instead of `404`.
    ServeDir::new(dir).fallback(index_fallback)
}

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
        .nest("/api/data_view", routes::data_view::router(state.clone()))
        .route(
            "/api/warehouse-type",
            get(routes::meta::warehouse_type_handler),
        )
        .route("/api/get_user", get(routes::meta::get_user_handler))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .fallback_service(spa_service())
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
