use axum::{Json, Router, routing::get};
use serde_json::json;

use crate::state::AppState;
use common_rs::auth::AuthenticatedUser;

pub mod allocation_plan;
pub mod approval_cube;
pub mod input_health;
pub mod input_plans;
pub mod kpis;
pub mod metadata;
pub mod minimum_allocations;
pub mod output_overview;
pub mod output_runs;
pub mod service_levels;
pub mod workflow;

pub fn assemble_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/api/get_user", get(get_user))
        .nest("/api", api_router())
}

fn api_router() -> Router<AppState> {
    Router::new()
        .merge(metadata::router())
        .merge(input_plans::router())
        .merge(input_health::router())
        .merge(minimum_allocations::router())
        .merge(kpis::router())
        .merge(output_overview::router())
        .merge(output_runs::router())
        .merge(workflow::router())
        .merge(approval_cube::router())
        .merge(service_levels::router())
        .merge(allocation_plan::router())
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "app": "weekly" }))
}

async fn get_user(AuthenticatedUser(user): AuthenticatedUser) -> Json<serde_json::Value> {
    Json(json!({
        "email": user.email,
        "role": user.role,
        "can_run_workflow": user.can_run_workflow,
        "can_edit_plan": user.can_edit_plan,
    }))
}
