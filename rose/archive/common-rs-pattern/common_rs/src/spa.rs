//! Shared SPA fallback. Mirrors the python `serve_spa` semantics:
//! - `/api/*` paths that didn't match a registered route return 404 JSON
//!   (so missing API endpoints don't silently fall through to index.html).
//! - All other unmatched paths serve the file from `dist/` if it exists,
//!   otherwise fall back to `dist/index.html` (client-side routing).
//!
//! ## Wiring
//!
//! The dist path travels through an `axum::Extension` because
//! `env!("CARGO_MANIFEST_DIR")` resolves to *this crate* at compile time and
//! cannot be used to locate the consuming app's `frontend/dist`. Each app
//! computes its own dev fallback and passes it in:
//!
//! ```ignore
//! let dist = common_rs::spa::DistPath::resolve(
//!     PathBuf::from(env!("CARGO_MANIFEST_DIR"))
//!         .parent().unwrap().join("frontend").join("dist"),
//! );
//! let router = router
//!     .fallback(common_rs::spa::fallback)
//!     .layer(axum::Extension(dist));
//! ```

use std::path::PathBuf;

use axum::{
    Extension, Json,
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

/// Where to read the built SPA bundle from. The Dockerfile sets `DIST_PATH`
/// because the binary's compile-time manifest dir doesn't exist in the
/// runtime image.
#[derive(Clone, Debug)]
pub struct DistPath(pub PathBuf);

impl DistPath {
    /// Read `DIST_PATH` from the environment if set, else use the supplied
    /// development default (typically the consumer crate's
    /// `../frontend/dist`).
    pub fn resolve(dev_default: PathBuf) -> Self {
        let path = std::env::var("DIST_PATH")
            .map(PathBuf::from)
            .unwrap_or(dev_default);
        Self(path)
    }
}

pub async fn fallback(Extension(DistPath(dist)): Extension<DistPath>, req: Request) -> Response {
    let path = req.uri().path();

    if path.starts_with("/api/") {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not Found" })),
        )
            .into_response();
    }

    let serve = ServeDir::new(&dist).fallback(ServeFile::new(dist.join("index.html")));

    match serve.oneshot(req).await {
        Ok(resp) => resp.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "spa serve error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "detail": "spa serve error" })),
            )
                .into_response()
        }
    }
}
