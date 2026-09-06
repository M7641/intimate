use utoipa::OpenApi;

use crate::routes;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Data View API",
        description = "Snapshot data exploration API",
        version = "0.1.0"
    ),
    tags(
        (name = "Health", description = "Service health checks and observability"),
        (name = "Data View - Browse", description = "Schema and table discovery"),
        (name = "Data View - Inspect", description = "Table-level inspection (columns, timestamps, row counts)"),
        (name = "Data View - Data", description = "Row-level data retrieval with filtering"),
        (name = "Data View - Column Analysis", description = "Column statistics and value distributions"),
        (name = "Data View - Table Metadata", description = "Comprehensive table metadata including backend-specific details"),
        (name = "Data View - Database", description = "Whole-database catalogue map and inferred relationships"),
        (name = "Meta", description = "Warehouse type and user info"),
    ),
    paths(
        // Health & observability
        routes::health::liveness,
        routes::health::readiness,
        routes::health::metrics_handler,
        // Browse
        routes::data_view::browse::schemas_handler,
        routes::data_view::browse::tables_handler,
        routes::data_view::browse::recent_loads_handler,
        routes::data_view::browse::db_info_handler,
        // Inspect
        routes::data_view::inspect::columns_handler,
        routes::data_view::inspect::timestamps_handler,
        routes::data_view::inspect::row_counts_handler,
        // Data
        routes::data_view::data::handler,
        // Column analysis
        routes::data_view::column::values_handler,
        routes::data_view::column::stats_handler,
        routes::data_view::column::distribution_handler,
        // Table metadata
        routes::data_view::table_meta::handler,
        // Database catalogue
        routes::data_view::catalogue::catalogue_handler,
        routes::data_view::catalogue::link_overlap_handler,
        routes::data_view::health::handler,
        // Meta
        routes::meta::warehouse_type_handler,
        routes::meta::get_user_handler,
    ),
    components(schemas(
        service_kit::error::ErrorResponse,
        // Browse
        routes::data_view::browse::DbInfoResponse,
        routes::data_view::browse::RecentLoad,
        // Inspect
        routes::data_view::inspect::ColumnInfoResp,
        routes::data_view::inspect::SnapshotTimestamp,
        routes::data_view::inspect::TimestampRowCount,
        // Data
        routes::data_view::data::DataFilter,
        // Column analysis
        routes::data_view::column::ColumnStatsResponse,
        routes::data_view::column::ValueDistributionResponse,
        routes::data_view::column::ValueDistributionItem,
        // Table metadata
        routes::data_view::table_meta::TableMetaResponse,
        routes::data_view::table_meta::ColumnDetail,
        routes::data_view::table_meta::RedshiftTableStorage,
        routes::data_view::table_meta::SnowflakeTableDetails,
        // Database catalogue
        routes::data_view::catalogue::CatalogueResponse,
        routes::data_view::catalogue::CatalogueNode,
        routes::data_view::catalogue::CatalogueEdge,
        routes::data_view::catalogue::EdgeColumn,
        routes::data_view::catalogue::EdgeKind,
        routes::data_view::catalogue::OmittedColumn,
        routes::data_view::catalogue::LinkOverlapResponse,
        routes::data_view::health::SchemaHealthResponse,
        routes::data_view::health::TableHealth,
    ))
)]
pub struct ApiDoc;
