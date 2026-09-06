//! Snowflake analytics endpoints, served flat under `/api/*`. One module per
//! path — each owns its handler, its SQL (`query`), and any endpoint-specific
//! params + tests. Cross-cutting bits (validation limits, the credit-rate CASE,
//! shared param structs) live in [`shared`]; the shared response shape in
//! [`response`].

pub mod response;
mod shared;

// Published to `main` so startup can point the analytics endpoints at the
// verified query-history cache table (Phase 2 — no information_schema fallback).
pub use shared::set_history_source;

// One module per path. `pub(crate)` so `openapi.rs` can name each handler.
pub(crate) mod compilation_analysis;
pub(crate) mod cost_by_query_type;
pub(crate) mod cost_by_table;
// DISABLED: /credits-by-day is too slow on INFORMATION_SCHEMA and times out (504).
// Route + OpenAPI entry are commented out; the module is kept so it can be
// re-enabled once fast (e.g. moved to ACCOUNT_USAGE). `allow(dead_code)` because
// nothing references it while it is off.
#[allow(dead_code)]
pub(crate) mod credits_by_day;
pub(crate) mod credits_by_tag;
pub(crate) mod expensive_queries;
pub(crate) mod failed_queries;
pub(crate) mod info;
pub(crate) mod query_frequency;
pub(crate) mod query_plan;
pub(crate) mod queue_analysis;
pub(crate) mod recent_queries;
pub(crate) mod repeated_queries;
pub(crate) mod table_storage;
pub(crate) mod warehouse_utilization;

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
        .route("/info", get(info::handler))
        .route("/expensive-queries", get(expensive_queries::handler))
        .route("/cost-by-query-type", get(cost_by_query_type::handler))
        .route("/cost-by-table", get(cost_by_table::handler))
        // DISABLED (too slow — times out). Re-add once credits_by_day is fast:
        // .route("/credits-by-day", get(credits_by_day::handler))
        .route("/credits-by-tag", get(credits_by_tag::handler))
        .route(
            "/warehouse-utilization",
            get(warehouse_utilization::handler),
        )
        .route("/queue-analysis", get(queue_analysis::handler))
        .route("/compilation-analysis", get(compilation_analysis::handler))
        .route("/table-storage", get(table_storage::handler))
        .route("/failed-queries", get(failed_queries::handler))
        .route("/repeated-queries", get(repeated_queries::handler))
        .route("/query-frequency", get(query_frequency::handler))
        .route("/recent-queries", get(recent_queries::handler))
        .route("/query-plan", get(query_plan::handler))
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
/// This app is Snowflake-only, so the type is constant.
#[utoipa::path(
    get,
    path = "/api/warehouse-type",
    tag = "Warehouse",
    responses((status = 200, description = "Active warehouse type", body = serde_json::Value))
)]
pub async fn warehouse_type_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "warehouse_type": "snowflake" }))
}

/// Get current user info
#[utoipa::path(
    get,
    path = "/api/get_user",
    tag = "Warehouse",
    responses((status = 200, description = "User and email info", body = serde_json::Value))
)]
pub async fn get_user_handler(headers: HeaderMap) -> Json<serde_json::Value> {
    // Identify the *requester*, never the server. Reading the container's `USER`
    // env would leak backend identity into a customer-facing tool; instead we
    // derive everything from the auth header the gateway injects.
    let email = headers
        .get("X-Auth-Email")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();

    // The user handle is the local part of the email (before `@`), or empty.
    let user = email.split('@').next().unwrap_or_default().to_string();

    Json(serde_json::json!({
        "user": user,
        "email": email,
    }))
}
