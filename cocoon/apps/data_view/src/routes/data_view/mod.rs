pub mod browse;
pub mod catalogue;
pub mod column;
pub mod data;
pub mod health;
pub mod inspect;
pub mod table_meta;
pub mod validation;

use axum::Router;
use axum::routing::get;

use service_kit::middleware;
use service_kit::state::AppState;

/// Build the /api/data_view sub-router.
///
/// Routes are split into light (metadata/browse) and heavy (full data scans).
/// Heavy routes get an extended timeout and a stricter rate limit.
pub fn router(state: AppState) -> Router<AppState> {
    // Heavy routes — longer timeout, separate rate limit
    let heavy = Router::new()
        .route("/data/{table_name}", get(data::handler))
        .route(
            "/column_values/{table_name}/{column_name}",
            get(column::values_handler),
        )
        .route(
            "/value_distribution/{table_name}/{column_name}",
            get(column::distribution_handler),
        )
        // Overlap validation scans distinct values across two tables — heavy.
        .route("/link_overlap", get(catalogue::link_overlap_handler))
        // Schema health counts rows across every table's snapshots — heavy.
        .route("/schema_health", get(health::handler))
        .layer(axum::middleware::from_fn(middleware::heavy_route_timeout))
        .layer(axum::middleware::from_fn_with_state(
            state,
            middleware::heavy_rate_limit,
        ));

    // Light routes — use the global timeout and rate limit
    Router::new()
        .route("/schemas", get(browse::schemas_handler))
        .route("/tables", get(browse::tables_handler))
        .route("/recent_loads", get(browse::recent_loads_handler))
        .route("/db_info", get(browse::db_info_handler))
        .route("/columns/{table_name}", get(inspect::columns_handler))
        .route("/timestamps/{table_name}", get(inspect::timestamps_handler))
        .route("/row_counts/{table_name}", get(inspect::row_counts_handler))
        .route(
            "/column_stats/{table_name}/{column_name}",
            get(column::stats_handler),
        )
        .route("/table_meta/{table_name}", get(table_meta::handler))
        // Catalogue map: metadata-only graph, cheap — light.
        .route("/catalogue", get(catalogue::catalogue_handler))
        .merge(heavy)
}
