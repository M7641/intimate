use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use tracing::info;

use super::validation::validate_schema_name;

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct DbInfoResponse {
    pub backend: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SchemaQueryParam {
    #[serde(default = "default_schema")]
    pub schema: String,
}

fn default_schema() -> String {
    "stage".to_string()
}

/// One table's most recent load, for the home "recently updated" overview.
#[derive(Debug, Serialize, ToSchema)]
pub struct RecentLoad {
    pub schema: String,
    pub table_name: String,
    /// ISO timestamp of the latest load, or `null` if the table is empty.
    pub last_loaded: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct RecentLoadsParams {
    #[serde(default = "default_schema")]
    pub schema: String,
    #[serde(default = "default_recent_limit")]
    pub limit: u32,
}

fn default_recent_limit() -> u32 {
    8
}

// ── Query builders ────────────────────────────────────────────────────

/// Build SQL to discover available schemas (excluding system schemas).
fn build_schemas_query() -> String {
    "SELECT DISTINCT table_schema \
     FROM information_schema.tables \
     WHERE lower(table_schema) NOT IN ('information_schema', 'pg_catalog', 'pg_internal') \
     ORDER BY table_schema"
        .to_string()
}

/// Build SQL to discover all tables in a given schema.
fn build_tables_query(schema: &str) -> String {
    let schema_lower = schema.to_lowercase();
    format!(
        "SELECT table_name FROM information_schema.tables \
         WHERE lower(table_schema) = '{schema_lower}' \
         ORDER BY table_name"
    )
}

/// Build SQL listing the schema's tables that carry a `load_timestamp` column.
fn build_load_tables_query(schema: &str) -> String {
    let schema_lower = schema.to_lowercase();
    format!(
        "SELECT table_name FROM information_schema.columns \
         WHERE lower(table_schema) = '{schema_lower}' \
         AND lower(column_name) = 'load_timestamp' \
         ORDER BY table_name"
    )
}

/// Build a single query that returns the latest load per table by UNION-ing a
/// cheap `MAX(load_timestamp)` over each table, then sorting newest-first.
fn build_recent_loads_query(schema: &str, tables: &[String], limit: u32) -> String {
    let selects: Vec<String> = tables
        .iter()
        .map(|t| {
            let label = t.replace('\'', "''");
            format!(
                "SELECT '{label}' AS table_name, \
                 CAST(MAX(load_timestamp) AS VARCHAR) AS last_loaded \
                 FROM {schema}.\"{t}\""
            )
        })
        .collect();

    format!(
        "SELECT * FROM ({}) t ORDER BY last_loaded DESC LIMIT {limit}",
        selects.join(" UNION ALL ")
    )
}

// ── Handlers ──────────────────────────────────────────────────────────

/// List available schemas
#[utoipa::path(
    get,
    path = "/api/data_view/schemas",
    tag = "Data View - Browse",
    responses(
        (status = 200, description = "List of schema names", body = Vec<String>),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn schemas_handler(State(state): State<AppState>) -> Result<Json<Vec<String>>, AppError> {
    let sql = build_schemas_query();
    let rows = state.blocking_query(sql).await?;

    info!("Executing SQL to fetch schemas: {:?}", rows);

    let schemas: Vec<String> = rows
        .into_iter()
        .filter_map(|row| {
            row.get("table_schema")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    Ok(Json(schemas))
}

/// List tables in a schema
#[utoipa::path(
    get,
    path = "/api/data_view/tables",
    tag = "Data View - Browse",
    params(SchemaQueryParam),
    responses(
        (status = 200, description = "List of table names", body = Vec<String>),
        (status = 400, description = "Invalid schema name", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn tables_handler(
    State(state): State<AppState>,
    Query(params): Query<SchemaQueryParam>,
) -> Result<Json<Vec<String>>, AppError> {
    validate_schema_name(&params.schema)?;

    let sql = build_tables_query(&params.schema);
    let rows = state.blocking_query(sql).await?;

    let tables: Vec<String> = rows
        .into_iter()
        .filter_map(|row| {
            row.get("table_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    Ok(Json(tables))
}

/// List the most recently loaded tables in a schema
#[utoipa::path(
    get,
    path = "/api/data_view/recent_loads",
    tag = "Data View - Browse",
    params(RecentLoadsParams),
    responses(
        (status = 200, description = "Recently loaded tables, newest first", body = Vec<RecentLoad>),
        (status = 400, description = "Invalid schema name", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn recent_loads_handler(
    State(state): State<AppState>,
    Query(params): Query<RecentLoadsParams>,
) -> Result<Json<Vec<RecentLoad>>, AppError> {
    validate_schema_name(&params.schema)?;
    let limit = params.limit.clamp(1, 50);

    // 1. Which tables in this schema even have a load_timestamp?
    let tables_sql = build_load_tables_query(&params.schema);
    let table_rows = state.blocking_query(tables_sql).await?;
    let tables: Vec<String> = table_rows
        .into_iter()
        .filter_map(|row| {
            row.get("table_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        // Only safe identifiers get interpolated into the UNION query.
        .filter(|t| validate_schema_name(t).is_ok())
        .collect();

    if tables.is_empty() {
        return Ok(Json(vec![]));
    }

    // 2. One query: latest load per table, newest first.
    let sql = build_recent_loads_query(&params.schema, &tables, limit);
    let rows = state.blocking_query(sql).await?;

    let result: Vec<RecentLoad> = rows
        .into_iter()
        .map(|row| RecentLoad {
            schema: params.schema.clone(),
            table_name: row
                .get("table_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            last_loaded: row
                .get("last_loaded")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
        .collect();

    Ok(Json(result))
}

/// Get database backend info
#[utoipa::path(
    get,
    path = "/api/data_view/db_info",
    tag = "Data View - Browse",
    responses((status = 200, description = "Database backend type", body = DbInfoResponse))
)]
pub async fn db_info_handler(State(state): State<AppState>) -> Json<DbInfoResponse> {
    let backend = match state.backend() {
        "amazon_redshift" => "redshift",
        other => other,
    };
    Json(DbInfoResponse {
        backend: backend.to_string(),
    })
}
