use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::validation::{
    EXCLUDED_COLUMNS, table_has_load_timestamp, validate_schema_name, validate_table_name,
};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct ColumnInfoResp {
    pub column_name: String,
    pub data_type: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SnapshotTimestamp {
    pub load_timestamp: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TimestampRowCount {
    pub load_timestamp: String,
    pub row_count: i64,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SchemaQueryParam {
    #[serde(default = "default_schema")]
    pub schema: String,
}

fn default_schema() -> String {
    "stage".to_string()
}

// ── Query builders ────────────────────────────────────────────────────

fn build_columns_query(schema: &str, table: &str) -> String {
    let schema_lower = schema.to_lowercase();
    let table_lower = table.to_lowercase();
    format!(
        "SELECT column_name, data_type \
         FROM information_schema.columns \
         WHERE lower(table_schema) = '{schema_lower}' \
         AND lower(table_name) = '{table_lower}' \
         ORDER BY ordinal_position"
    )
}

fn build_timestamps_query(schema: &str, table: &str) -> String {
    format!(
        "SELECT DISTINCT load_timestamp::varchar AS load_timestamp \
         FROM {schema}.\"{table}\" \
         ORDER BY load_timestamp DESC \
         LIMIT 50"
    )
}

fn build_row_counts_query(schema: &str, table: &str) -> String {
    // Cap to the 50 most recent snapshots (matching the timestamps endpoint).
    // Without a limit, a frequently-loaded table returns thousands of points —
    // mostly the same row_count repeated — which is wasteful and reads as
    // duplicated data. Take the newest 50, then re-order ascending for the chart.
    format!(
        "SELECT load_timestamp, row_count FROM ( \
         SELECT load_timestamp::varchar AS load_timestamp, COUNT(*) AS row_count \
         FROM {schema}.\"{table}\" \
         GROUP BY load_timestamp \
         ORDER BY load_timestamp DESC \
         LIMIT 50 \
         ) recent \
         ORDER BY load_timestamp ASC"
    )
}

// ── Handlers ──────────────────────────────────────────────────────────

/// Get columns for a table
#[utoipa::path(
    get,
    path = "/api/data_view/columns/{table_name}",
    tag = "Data View - Inspect",
    params(
        ("table_name" = String, Path, description = "Name of the table"),
        SchemaQueryParam,
    ),
    responses(
        (status = 200, description = "Column names and types", body = Vec<ColumnInfoResp>),
        (status = 400, description = "Invalid table or schema name", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state), fields(table = %table_name))]
pub async fn columns_handler(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Query(params): Query<SchemaQueryParam>,
) -> Result<Json<Vec<ColumnInfoResp>>, AppError> {
    validate_schema_name(&params.schema)?;
    validate_table_name(&table_name)?;

    let sql = build_columns_query(&params.schema, &table_name);
    let rows = state.blocking_query(sql).await?;

    let columns: Vec<ColumnInfoResp> = rows
        .into_iter()
        .filter_map(|row| {
            let name = row.get("column_name")?.as_str()?.to_string();
            if EXCLUDED_COLUMNS.contains(name.as_str()) {
                return None;
            }
            let data_type = row.get("data_type")?.as_str()?.to_string();
            Some(ColumnInfoResp {
                column_name: name,
                data_type,
            })
        })
        .collect();

    Ok(Json(columns))
}

/// Get snapshot timestamps for a table
#[utoipa::path(
    get,
    path = "/api/data_view/timestamps/{table_name}",
    tag = "Data View - Inspect",
    params(
        ("table_name" = String, Path, description = "Name of the table"),
        SchemaQueryParam,
    ),
    responses(
        (status = 200, description = "Available snapshot timestamps (up to 50 most recent)", body = Vec<SnapshotTimestamp>),
        (status = 400, description = "Invalid table or schema name", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state), fields(table = %table_name))]
pub async fn timestamps_handler(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Query(params): Query<SchemaQueryParam>,
) -> Result<Json<Vec<SnapshotTimestamp>>, AppError> {
    validate_schema_name(&params.schema)?;
    validate_table_name(&table_name)?;

    if !table_has_load_timestamp(&state, &params.schema, &table_name).await? {
        return Ok(Json(vec![]));
    }

    let sql = build_timestamps_query(&params.schema, &table_name);
    let rows = state.blocking_query(sql).await?;

    let timestamps: Vec<SnapshotTimestamp> = rows
        .into_iter()
        .filter_map(|row| {
            row.get("load_timestamp")
                .and_then(|v| v.as_str())
                .map(|s| SnapshotTimestamp {
                    load_timestamp: s.to_string(),
                })
        })
        .collect();

    Ok(Json(timestamps))
}

/// Get row counts per snapshot timestamp
#[utoipa::path(
    get,
    path = "/api/data_view/row_counts/{table_name}",
    tag = "Data View - Inspect",
    params(
        ("table_name" = String, Path, description = "Name of the table"),
        SchemaQueryParam,
    ),
    responses(
        (status = 200, description = "Row count per snapshot timestamp", body = Vec<TimestampRowCount>),
        (status = 400, description = "Invalid table or schema name", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state), fields(table = %table_name))]
pub async fn row_counts_handler(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Query(params): Query<SchemaQueryParam>,
) -> Result<Json<Vec<TimestampRowCount>>, AppError> {
    validate_schema_name(&params.schema)?;
    validate_table_name(&table_name)?;

    if !table_has_load_timestamp(&state, &params.schema, &table_name).await? {
        return Ok(Json(vec![]));
    }

    let sql = build_row_counts_query(&params.schema, &table_name);
    let rows = state.blocking_query(sql).await?;

    let counts: Vec<TimestampRowCount> = rows
        .into_iter()
        .filter_map(|row| {
            let ts = row.get("load_timestamp")?.as_str()?.to_string();
            let count = row.get("row_count")?.as_i64()?;
            Some(TimestampRowCount {
                load_timestamp: ts,
                row_count: count,
            })
        })
        .collect();

    Ok(Json(counts))
}
