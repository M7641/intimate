use utoipa::OpenApi;

use crate::routes;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Snowhouse API",
        description = "Snowflake analytics and cost insights API",
        version = "0.1.0"
    ),
    tags(
        (name = "Health", description = "Service health checks and observability"),
        (name = "Warehouse", description = "Warehouse type and user info"),
        (name = "Snowflake", description = "Snowflake analytics and cost insights"),
    ),
    paths(
        // Health & observability
        routes::health::liveness,
        routes::health::readiness,
        routes::health::metrics_handler,
        // Warehouse
        routes::analytics::warehouse_type_handler,
        routes::analytics::get_user_handler,
        // Snowflake
        routes::analytics::info::handler,
        routes::analytics::expensive_queries::handler,
        routes::analytics::cost_by_query_type::handler,
        routes::analytics::cost_by_table::handler,
        // DISABLED: /credits-by-day is too slow and times out — see its module.
        // routes::analytics::credits_by_day::handler,
        routes::analytics::credits_by_tag::handler,
        routes::analytics::warehouse_utilization::handler,
        routes::analytics::queue_analysis::handler,
        routes::analytics::compilation_analysis::handler,
        routes::analytics::table_storage::handler,
        routes::analytics::failed_queries::handler,
        routes::analytics::repeated_queries::handler,
        routes::analytics::query_frequency::handler,
        routes::analytics::recent_queries::handler,
        routes::analytics::query_plan::handler,
    ),
    components(schemas(
        service_kit::error::ErrorResponse,
        // Warehouse
        routes::analytics::response::WarehouseResponse,
    ))
)]
pub struct ApiDoc;
