//! `GET /api/table-storage` — per-table storage
//! consumption from `INFORMATION_SCHEMA.TABLES`.

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::routes::validation::validate_schema_name;
use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::response::WarehouseResponse;

/// Optional `?database=&schema=` filters on the storage listing.
#[derive(Deserialize, IntoParams)]
pub struct TableStorageParams {
    pub database: Option<String>,
    pub schema: Option<String>,
}

pub fn query(database: Option<&str>, schema: Option<&str>) -> String {
    let db_clause = database
        .map(|d| format!("AND table_catalog = '{d}'"))
        .unwrap_or_default();
    let schema_clause = schema
        .map(|s| format!("AND table_schema = '{s}'"))
        .unwrap_or_default();

    format!(
        "SELECT
            table_catalog as database_name,
            table_schema as schema_name,
            table_name,
            row_count,
            ROUND(bytes / (1024 * 1024), 2) as size_mb,
            ROUND(bytes / (1024 * 1024 * 1024), 4) as size_gb,
            retention_time,
            TO_CHAR(created, 'YYYY-MM-DD HH24:MI:SS') as table_created,
            TO_CHAR(last_altered, 'YYYY-MM-DD HH24:MI:SS') as last_altered,
            table_type,
            clustering_key,
            is_transient,
            auto_clustering_on
        FROM INFORMATION_SCHEMA.TABLES
        WHERE table_schema != 'INFORMATION_SCHEMA'
            {db_clause}
            {schema_clause}
        ORDER BY bytes DESC NULLS LAST"
    )
}

/// Get table storage consumption
#[utoipa::path(get, path = "/api/table-storage", tag = "Snowflake",
    params(TableStorageParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<TableStorageParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    if let Some(ref d) = params.database {
        validate_schema_name(d)?; // same identifier rules apply
    }
    if let Some(ref s) = params.schema {
        validate_schema_name(s)?;
    }
    let sql = query(params.database.as_deref(), params.schema.as_deref());
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}
