use axum::routing::get;
use axum::Json;
use axum::Router;
use serde::Serialize;
use std::path::PathBuf;
use tower_http::services::ServeDir;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Serialize)]
pub struct InfoResponse {
    pub name: String,
    pub description: String,
    pub stack: Vec<String>,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

pub async fn info() -> Json<InfoResponse> {
    Json(InfoResponse {
        name: "nix_stack".into(),
        description: "Nix-reproducible multi-language stack".into(),
        stack: vec![
            "Rust (Axum)".into(),
            "SolidJS (Vite)".into(),
            "Python (Typer CLI)".into(),
            "Nix (flake)".into(),
        ],
    })
}

/// Build the application router.
///
/// When `static_dir` is `Some`, the frontend's built `dist/` directory is served
/// as a fallback after API routes — enabling SPA routing.
/// When `None` (development), only API routes are active.
pub fn app(static_dir: Option<PathBuf>) -> Router {
    let api = Router::new()
        .route("/api/health", get(health))
        .route("/api/info", get(info));

    match static_dir {
        Some(dir) => {
            let serve = ServeDir::new(dir).append_index_html_on_directories(true);
            api.fallback_service(serve)
        }
        None => api,
    }
}
