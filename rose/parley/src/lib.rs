//! parley — a conversational language-learning companion.
//!
//! lib.rs holds the router so integration tests can build the app without binding
//! a port (via `tower::ServiceExt::oneshot`). main.rs is just the entry point.

pub mod areas;
pub mod domain;
pub mod memory;
pub mod pedagogy;
pub mod providers;
pub mod routes;
pub mod state;

use std::path::PathBuf;

use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// Locate the built frontend bundle (the directory holding `index.html`).
///
/// `ServeDir` resolves paths against the process's working directory, which
/// differs across run modes, so we probe candidates in priority order and return
/// the first that actually holds a bundle:
///
/// 1. `PARLEY_STATIC_DIR` — explicit override, wins if set.
/// 2. `frontend/dist` relative to the CWD — a run started from the crate root.
/// 3. `<crate>/frontend/dist` baked at compile time — a run started elsewhere.
///
/// Returns `None` when no bundle is found (API-only dev, where Vite serves the UI).
pub fn spa_bundle_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var("PARLEY_STATIC_DIR").ok().map(PathBuf::from),
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

/// Serve the built SPA: real files from the bundle, and `index.html` (with a
/// `200`) for any unmatched path so a direct visit or refresh resolves instead
/// of 404-ing. When no bundle exists the fallback returns `404` — API-only dev,
/// where the UI lives on the Vite server.
fn spa_service() -> ServeDir<axum::routing::MethodRouter> {
    let dir = spa_bundle_dir().unwrap_or_else(|| PathBuf::from("frontend/dist"));
    let index_html = std::fs::read_to_string(dir.join("index.html")).ok();

    let index_fallback = get(move || {
        let index_html = index_html.clone();
        async move {
            match index_html {
                Some(html) => Html(html).into_response(),
                None => {
                    (axum::http::StatusCode::NOT_FOUND, "Frontend bundle not found").into_response()
                }
            }
        }
    });

    ServeDir::new(dir).fallback(index_fallback)
}

/// Build the application router. `/api` and `/health` are handled directly; any
/// other path falls through to the built SPA (or 404 in API-only dev). CORS is
/// permissive for local dev, where the Vite dev server runs on a different port.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(routes::health))
        .route("/api/chat", post(routes::chat))
        .route("/api/converse", post(routes::converse))
        .route("/api/progress", get(routes::progress))
        .route("/api/words", get(routes::list_words).post(routes::save_word))
        .fallback_service(spa_service())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
