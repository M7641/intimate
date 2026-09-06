use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;

use service_kit::state::AppState;

/// Get active warehouse type.
///
/// The data viewer is warehouse-agnostic, but the frontend still uses this to
/// tailor backend-specific display (e.g. Redshift vs Snowflake table metadata).
#[utoipa::path(
    get,
    path = "/api/warehouse-type",
    tag = "Meta",
    responses((status = 200, description = "Active warehouse type", body = serde_json::Value))
)]
pub async fn warehouse_type_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let wt = match state.backend() {
        "amazon_redshift" => "redshift",
        other => other,
    };
    // `connector` disambiguates which driver actually serves requests (the
    // native Postgres/Snowflake connector vs the ADBC Arrow driver), since the
    // warehouse type alone doesn't say.
    Json(serde_json::json!({
        "warehouse_type": wt,
        "connector": state.connector().label(),
    }))
}

/// Get current user info.
#[utoipa::path(
    get,
    path = "/api/get_user",
    tag = "Meta",
    responses((status = 200, description = "User and email info", body = serde_json::Value))
)]
pub async fn get_user_handler(headers: HeaderMap) -> Json<serde_json::Value> {
    let user = std::env::var("USER").unwrap_or_else(|_| "local_user".to_string());

    let email = headers
        .get("X-Auth-Email")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();

    Json(serde_json::json!({
        "user": user,
        "email": email,
    }))
}
