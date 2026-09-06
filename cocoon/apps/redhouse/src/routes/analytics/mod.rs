//! Redshift analytics endpoints, served flat under `/api/*`. Handlers live in
//! [`handlers`], their SQL in [`queries`], the shared response shape in
//! [`response`].

pub mod handlers;
mod queries;
pub mod response;

use axum::Json;
use axum::Router;
use axum::http::HeaderMap;
use axum::routing::get;

use service_kit::middleware;
use service_kit::state::AppState;

/// Build the `/api` sub-router — all analytics endpoints plus the two light meta
/// endpoints.
///
/// The analytics endpoints scan query history and metadata catalogs and are
/// heavy by nature, so they get the extended timeout and the stricter heavy rate
/// limit. The meta endpoints (warehouse type, current user) are merged in
/// without those layers.
pub fn router(state: AppState) -> Router<AppState> {
    let heavy = Router::new()
        .route("/info", get(handlers::info_handler))
        .route("/table-scans", get(handlers::table_scans_handler))
        .route(
            "/query-performance",
            get(handlers::query_performance_handler),
        )
        .route("/compression", get(handlers::compression_handler))
        .route("/access-patterns", get(handlers::access_patterns_handler))
        .route("/slow-queries", get(handlers::slow_queries_handler))
        .route("/unused-tables", get(handlers::unused_tables_handler))
        .route("/disk-queries", get(handlers::disk_queries_handler))
        .route("/table-sizes", get(handlers::table_sizes_handler))
        .route("/storage", get(handlers::storage_handler))
        .route("/user-activity", get(handlers::user_activity_handler))
        .route("/distribution", get(handlers::distribution_handler))
        .route(
            "/filter-effectiveness",
            get(handlers::filter_effectiveness_handler),
        )
        .route("/query-frequency", get(handlers::query_frequency_handler))
        .route("/recent-queries", get(handlers::recent_queries_handler))
        .route("/query-plan", get(handlers::query_plan_handler))
        .layer(axum::middleware::from_fn(middleware::heavy_route_timeout))
        .layer(axum::middleware::from_fn_with_state(
            state,
            middleware::heavy_rate_limit,
        ));

    let meta = Router::new()
        .route("/warehouse-type", get(warehouse_type_handler))
        .route("/get_user", get(get_user_handler));

    heavy.merge(meta)
}

/// Get active warehouse type
///
/// This app is Redshift-only, so the type is constant.
#[utoipa::path(
    get,
    path = "/api/warehouse-type",
    tag = "Warehouse",
    responses((status = 200, description = "Active warehouse type", body = serde_json::Value))
)]
pub async fn warehouse_type_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "warehouse_type": "redshift" }))
}

/// Get current user info
#[utoipa::path(
    get,
    path = "/api/get_user",
    tag = "Warehouse",
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
