use utoipa::OpenApi;

use crate::routes;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Redhouse API",
        description = "Amazon Redshift analytics and performance insights API",
        version = "0.1.0"
    ),
    tags(
        (name = "Health", description = "Service health checks and observability"),
        (name = "Warehouse", description = "Warehouse type and user info"),
        (name = "Redshift", description = "Amazon Redshift analytics and performance insights"),
    ),
    paths(
        // Health & observability
        routes::health::liveness,
        routes::health::readiness,
        routes::health::metrics_handler,
        // Warehouse
        routes::analytics::warehouse_type_handler,
        routes::analytics::get_user_handler,
        // Redshift
        routes::analytics::handlers::info_handler,
        routes::analytics::handlers::table_scans_handler,
        routes::analytics::handlers::query_performance_handler,
        routes::analytics::handlers::compression_handler,
        routes::analytics::handlers::access_patterns_handler,
        routes::analytics::handlers::slow_queries_handler,
        routes::analytics::handlers::unused_tables_handler,
        routes::analytics::handlers::disk_queries_handler,
        routes::analytics::handlers::table_sizes_handler,
        routes::analytics::handlers::storage_handler,
        routes::analytics::handlers::user_activity_handler,
        routes::analytics::handlers::distribution_handler,
        routes::analytics::handlers::filter_effectiveness_handler,
        routes::analytics::handlers::query_frequency_handler,
        routes::analytics::handlers::recent_queries_handler,
        routes::analytics::handlers::query_plan_handler,
    ),
    components(schemas(
        service_kit::error::ErrorResponse,
        // Warehouse
        routes::analytics::response::WarehouseResponse,
    ))
)]
pub struct ApiDoc;
