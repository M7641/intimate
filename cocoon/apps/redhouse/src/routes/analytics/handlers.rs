use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::routes::validation::validate_schema_name;
use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::queries;
use super::response::WarehouseResponse;

/// Maximum value for `days` query parameters.
const MAX_DAYS: u32 = 365;
/// Maximum value for `limit` query parameters.
const MAX_LIMIT: u32 = 1000;

fn validate_days(days: u32) -> Result<(), AppError> {
    if days > MAX_DAYS {
        return Err(AppError::Validation(format!(
            "days must be <= {MAX_DAYS}, got {days}"
        )));
    }
    Ok(())
}

fn validate_limit(limit: u32) -> Result<(), AppError> {
    if limit > MAX_LIMIT {
        return Err(AppError::Validation(format!(
            "limit must be <= {MAX_LIMIT}, got {limit}"
        )));
    }
    Ok(())
}

// -- Query parameter structs --

#[derive(Debug, Deserialize, IntoParams)]
pub struct DaysParams {
    #[serde(default = "default_30")]
    pub days: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DaysSchemaParams {
    #[serde(default = "default_30")]
    pub days: u32,
    pub schema: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SchemaParams {
    pub schema: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SlowQueryParams {
    #[serde(default = "default_10")]
    pub min_seconds: u32,
    #[serde(default = "default_7")]
    pub days: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct UnusedTablesParams {
    #[serde(default = "default_30")]
    pub days_threshold: u32,
}

#[derive(Deserialize, IntoParams)]
pub struct DiskQueryParams {
    #[serde(default = "default_7")]
    pub days: u32,
}

#[derive(Deserialize, IntoParams)]
pub struct FilterEffectivenessParams {
    #[serde(default = "default_7")]
    pub days: u32,
    #[serde(default = "default_10")]
    pub min_scans: u32,
}

#[derive(Deserialize, IntoParams)]
pub struct FrequencyParams {
    #[serde(default = "default_7")]
    pub days: u32,
}

#[derive(Deserialize, IntoParams)]
pub struct RecentQueriesParams {
    #[serde(default = "default_7")]
    pub days: u32,
    #[serde(default = "default_50")]
    pub limit: u32,
}

#[derive(Deserialize, IntoParams)]
pub struct QueryPlanParams {
    pub query_id: i64,
}

fn default_7() -> u32 {
    7
}
fn default_10() -> u32 {
    10
}
fn default_30() -> u32 {
    30
}
fn default_50() -> u32 {
    50
}

// -- Handlers --

/// Get Redshift cluster info
#[utoipa::path(get, path = "/api/info", tag = "Redshift",
    responses((status = 200, description = "Redshift database info", body = serde_json::Value)))]
pub async fn info_handler() -> Json<serde_json::Value> {
    let db = std::env::var("REDSHIFT_DATABASE").unwrap_or_else(|_| "Unknown".to_string());
    Json(serde_json::json!({ "database": db }))
}

/// Get table scan statistics
#[utoipa::path(get, path = "/api/table-scans", tag = "Redshift",
    params(DaysSchemaParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn table_scans_handler(
    State(state): State<AppState>,
    Query(params): Query<DaysSchemaParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    if let Some(ref s) = params.schema {
        validate_schema_name(s)?;
    }
    let sql = queries::table_scans_query(params.days, params.schema.as_deref());
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get query performance metrics
#[utoipa::path(get, path = "/api/query-performance", tag = "Redshift",
    params(DaysParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn query_performance_handler(
    State(state): State<AppState>,
    Query(params): Query<DaysParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = queries::query_performance_query(params.days);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get column compression analysis
#[utoipa::path(get, path = "/api/compression", tag = "Redshift",
    params(SchemaParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn compression_handler(
    State(state): State<AppState>,
    Query(params): Query<SchemaParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    if let Some(ref s) = params.schema {
        validate_schema_name(s)?;
    }
    let sql = queries::compression_query(params.schema.as_deref());
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get access frequency patterns
#[utoipa::path(get, path = "/api/access-patterns", tag = "Redshift",
    params(DaysParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn access_patterns_handler(
    State(state): State<AppState>,
    Query(params): Query<DaysParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = queries::access_patterns_query(params.days);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get queries exceeding a duration threshold
#[utoipa::path(get, path = "/api/slow-queries", tag = "Redshift",
    params(SlowQueryParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn slow_queries_handler(
    State(state): State<AppState>,
    Query(params): Query<SlowQueryParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = queries::slow_queries_query(params.min_seconds, params.days);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get tables not accessed recently
#[utoipa::path(get, path = "/api/unused-tables", tag = "Redshift",
    params(UnusedTablesParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn unused_tables_handler(
    State(state): State<AppState>,
    Query(params): Query<UnusedTablesParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days_threshold)?;
    let (scan_sql, table_sql) = queries::unused_tables_queries(params.days_threshold);

    let scan_rows = state.blocking_query(scan_sql).await?;
    let table_rows = state.blocking_query(table_sql).await?;

    // Build a lookup from (schema, table) → last_accessed
    let mut scan_map = std::collections::HashMap::new();
    for row in scan_rows {
        let schema = row
            .get("schema_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let table = row
            .get("table_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let last_accessed = row
            .get("last_accessed")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        scan_map.insert((schema, table), last_accessed);
    }

    // Left join: all tables + scan info
    let mut joined = Vec::new();
    for row in table_rows {
        let schema = row
            .get("schema_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let table = row
            .get("table_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let mut out = row.clone();
        let key = (schema, table);

        match scan_map.get(&key) {
            Some(last_accessed) if !last_accessed.is_empty() => {
                out.insert(
                    "last_accessed".to_string(),
                    serde_json::Value::String(last_accessed.clone()),
                );
                // Determine status based on date comparison
                out.insert(
                    "status".to_string(),
                    serde_json::Value::String("Recently Used".to_string()),
                );
            }
            _ => {
                out.insert(
                    "last_accessed".to_string(),
                    serde_json::Value::String(format!(
                        "Not scanned in last {} days",
                        params.days_threshold
                    )),
                );
                out.insert(
                    "status".to_string(),
                    serde_json::Value::String("Potentially Unused".to_string()),
                );
            }
        }
        joined.push(out);
    }

    // Sort: unused tables first
    joined.sort_by(|a, b| {
        let a_status = a.get("status").and_then(|v| v.as_str()).unwrap_or_default();
        let b_status = b.get("status").and_then(|v| v.as_str()).unwrap_or_default();
        a_status.cmp(b_status)
    });

    Ok(Json(WarehouseResponse::from_rows(joined)))
}

/// Get queries with disk spills
#[utoipa::path(get, path = "/api/disk-queries", tag = "Redshift",
    params(DiskQueryParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn disk_queries_handler(
    State(state): State<AppState>,
    Query(params): Query<DiskQueryParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = queries::disk_queries_query(params.days);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get table size distribution
#[utoipa::path(get, path = "/api/table-sizes", tag = "Redshift",
    params(SchemaParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn table_sizes_handler(
    State(state): State<AppState>,
    Query(params): Query<SchemaParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    if let Some(ref s) = params.schema {
        validate_schema_name(s)?;
    }
    let sql = queries::table_sizes_query(params.schema.as_deref());
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get real on-disk storage per table (schema + table breakdown)
#[utoipa::path(get, path = "/api/storage", tag = "Redshift",
    params(SchemaParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn storage_handler(
    State(state): State<AppState>,
    Query(params): Query<SchemaParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    if let Some(ref s) = params.schema {
        validate_schema_name(s)?;
    }
    let sql = queries::storage_query(params.schema.as_deref());
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get user query activity
#[utoipa::path(get, path = "/api/user-activity", tag = "Redshift",
    params(DaysParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn user_activity_handler(
    State(state): State<AppState>,
    Query(params): Query<DaysParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = queries::user_activity_query(params.days);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get distribution key effectiveness
#[utoipa::path(get, path = "/api/distribution", tag = "Redshift",
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn distribution_handler(
    State(state): State<AppState>,
) -> Result<Json<WarehouseResponse>, AppError> {
    let sql = queries::distribution_query();
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get filter efficiency metrics
#[utoipa::path(get, path = "/api/filter-effectiveness", tag = "Redshift",
    params(FilterEffectivenessParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn filter_effectiveness_handler(
    State(state): State<AppState>,
    Query(params): Query<FilterEffectivenessParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = queries::filter_effectiveness_query(params.days, params.min_scans);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get query execution frequency
#[utoipa::path(get, path = "/api/query-frequency", tag = "Redshift",
    params(FrequencyParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn query_frequency_handler(
    State(state): State<AppState>,
    Query(params): Query<FrequencyParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = queries::query_frequency_query(params.days);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get recent query history
#[utoipa::path(get, path = "/api/recent-queries", tag = "Redshift",
    params(RecentQueriesParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn recent_queries_handler(
    State(state): State<AppState>,
    Query(params): Query<RecentQueriesParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    validate_limit(params.limit)?;
    let sql = queries::recent_queries_query(params.days, params.limit);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

/// Get query execution plan
#[utoipa::path(get, path = "/api/query-plan", tag = "Redshift",
    params(QueryPlanParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn query_plan_handler(
    State(state): State<AppState>,
    Query(params): Query<QueryPlanParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    let sql = queries::query_plan_query(params.query_id);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}
