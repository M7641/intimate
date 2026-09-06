//! `GET /api/info` — the connected Snowflake database name.

use axum::Json;

/// Get Snowflake account info
#[utoipa::path(get, path = "/api/info", tag = "Snowflake",
    responses((status = 200, description = "Snowflake database info", body = serde_json::Value)))]
pub async fn handler() -> Json<serde_json::Value> {
    let db = std::env::var("SNOWFLAKE_DATABASE").unwrap_or_else(|_| "Unknown".to_string());
    Json(serde_json::json!({ "database": db }))
}
