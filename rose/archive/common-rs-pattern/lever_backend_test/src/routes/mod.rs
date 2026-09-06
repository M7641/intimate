use axum::{Json, Router, routing::get};
use serde_json::json;

use crate::state::AppState;
use common_rs::auth::AuthenticatedUser;

pub mod distribution_point_post_codes;
pub mod mixed_pack_formats;
pub mod required_tooling;

pub fn assemble_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/api/get_user", get(get_user))
        .nest("/api", api_router())
}

fn api_router() -> Router<AppState> {
    Router::new()
        .merge(distribution_point_post_codes::router())
        .merge(mixed_pack_formats::router())
        .merge(required_tooling::router())
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "app": "levers" }))
}

async fn get_user(AuthenticatedUser(user): AuthenticatedUser) -> Json<serde_json::Value> {
    Json(json!({
        "email": user.email,
        "role": user.role,
        "can_run_workflow": user.can_run_workflow,
    }))
}
