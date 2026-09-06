use std::collections::HashSet;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::validation::{
    build_timestamp_where_clause, build_valid_columns_query, is_numeric_data_type,
    table_has_load_timestamp, validate_column_name, validate_schema_name, validate_table_name,
};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct ColumnStatsResponse {
    pub column_name: String,
    pub data_type: String,
    pub is_numeric: bool,
    pub total_count: i64,
    pub null_count: i64,
    pub null_pct: f64,
    pub distinct_count: i64,
    pub cardinality_pct: f64,
    /// Only present for numeric columns
    pub min_val: Option<f64>,
    pub max_val: Option<f64>,
    pub mean_val: Option<f64>,
    pub median_val: Option<f64>,
    pub stddev_val: Option<f64>,
    pub p25: Option<f64>,
    pub p75: Option<f64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ValueDistributionResponse {
    pub column_name: String,
    pub data_type: String,
    pub is_numeric: bool,
    pub distribution: Vec<ValueDistributionItem>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ValueDistributionItem {
    /// Categorical value (null for numeric distributions)
    pub value: Option<String>,
    /// Histogram bucket number (null for categorical)
    pub bucket: Option<i32>,
    pub bin_min: Option<f64>,
    pub bin_max: Option<f64>,
    pub count: i64,
}

// ── Query params ──────────────────────────────────────────────────────

fn default_schema() -> String {
    "stage".to_string()
}

#[derive(Deserialize, IntoParams)]
pub struct ColumnValuesParams {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub load_timestamp: Option<String>,
    #[serde(default = "default_values_limit")]
    pub limit: u32,
}

fn default_values_limit() -> u32 {
    500
}

#[derive(Deserialize, IntoParams)]
pub struct ColumnStatsParams {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub load_timestamp: Option<String>,
}

#[derive(Deserialize, IntoParams)]
pub struct ValueDistributionParams {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub load_timestamp: Option<String>,
    #[serde(default = "default_distribution_limit")]
    pub limit: u32,
}

fn default_distribution_limit() -> u32 {
    30
}

// ── Query builders ────────────────────────────────────────────────────

/// Build SQL to fetch the data type for a specific column.
fn build_column_type_query(schema: &str, table: &str, column: &str) -> String {
    let schema_lower = schema.to_lowercase();
    let table_lower = table.to_lowercase();
    let column_lower = column.to_lowercase();
    format!(
        "SELECT data_type \
         FROM information_schema.columns \
         WHERE lower(table_schema) = '{schema_lower}' \
         AND lower(table_name) = '{table_lower}' \
         AND lower(column_name) = '{column_lower}'"
    )
}

/// Build SQL to fetch distinct values for a specific column.
fn build_column_values_query(
    schema: &str,
    table: &str,
    column: &str,
    timestamp: Option<&str>,
    limit: u32,
    has_timestamp: bool,
) -> String {
    let where_clause = build_timestamp_where_clause(schema, table, timestamp, has_timestamp);

    format!(
        "SELECT DISTINCT CAST(\"{column}\" AS VARCHAR) AS value \
         FROM {schema}.\"{table}\" \
         {where_clause} \
         ORDER BY value \
         LIMIT {limit}"
    )
}

/// Build SQL for universal column statistics (works for all data types).
fn build_column_stats_query(
    schema: &str,
    table: &str,
    column: &str,
    timestamp: Option<&str>,
    has_timestamp: bool,
) -> String {
    let where_clause = build_timestamp_where_clause(schema, table, timestamp, has_timestamp);
    format!(
        "SELECT \
         COUNT(*) AS total_count, \
         COUNT(*) - COUNT(\"{column}\") AS null_count, \
         ROUND(100.0 * (COUNT(*) - COUNT(\"{column}\")) / NULLIF(COUNT(*), 0), 2) AS null_pct, \
         COUNT(DISTINCT \"{column}\") AS distinct_count, \
         ROUND(100.0 * COUNT(DISTINCT \"{column}\") / NULLIF(COUNT(*), 0), 2) AS cardinality_pct \
         FROM {schema}.\"{table}\" \
         {where_clause}"
    )
}

/// Build SQL for numeric column statistics (min, max, mean, median, etc.).
fn build_numeric_stats_query(
    schema: &str,
    table: &str,
    column: &str,
    timestamp: Option<&str>,
    has_timestamp: bool,
) -> String {
    let where_clause = build_timestamp_where_clause(schema, table, timestamp, has_timestamp);
    format!(
        "SELECT \
         MIN(\"{column}\"::FLOAT8) AS min_val, \
         MAX(\"{column}\"::FLOAT8) AS max_val, \
         AVG(\"{column}\"::FLOAT8) AS mean_val, \
         MEDIAN(\"{column}\"::FLOAT8) AS median_val, \
         STDDEV(\"{column}\"::FLOAT8) AS stddev_val, \
         PERCENTILE_CONT(0.25) WITHIN GROUP (ORDER BY \"{column}\"::FLOAT8) AS p25, \
         PERCENTILE_CONT(0.75) WITHIN GROUP (ORDER BY \"{column}\"::FLOAT8) AS p75 \
         FROM {schema}.\"{table}\" \
         {where_clause} AND \"{column}\" IS NOT NULL"
    )
}

/// Build SQL for categorical value distribution (top N values by count).
fn build_categorical_distribution_query(
    schema: &str,
    table: &str,
    column: &str,
    timestamp: Option<&str>,
    limit: u32,
    has_timestamp: bool,
) -> String {
    let where_clause = build_timestamp_where_clause(schema, table, timestamp, has_timestamp);
    format!(
        "SELECT COALESCE(CAST(\"{column}\" AS VARCHAR), '(null)') AS value, \
         COUNT(*) AS count \
         FROM {schema}.\"{table}\" \
         {where_clause} \
         GROUP BY 1 ORDER BY count DESC LIMIT {limit}"
    )
}

/// Build SQL for numeric histogram distribution (20 equal-width bins).
///
/// Buckets are computed arithmetically rather than with `WIDTH_BUCKET`, which
/// Amazon Redshift does not provide (its SQL forked from PostgreSQL 8.0.2,
/// predating that function). The formula `FLOOR((v - lo) / (hi - lo) * 20)`
/// yields bins 1..=20; values equal to the maximum are clamped into the last
/// bin, and a degenerate single-value column collapses to bin 1.
fn build_numeric_distribution_query(
    schema: &str,
    table: &str,
    column: &str,
    timestamp: Option<&str>,
    has_timestamp: bool,
) -> String {
    let where_clause = build_timestamp_where_clause(schema, table, timestamp, has_timestamp);
    format!(
        "WITH stats AS ( \
           SELECT MIN(\"{column}\"::FLOAT8) AS col_min, MAX(\"{column}\"::FLOAT8) AS col_max \
           FROM {schema}.\"{table}\" {where_clause} AND \"{column}\" IS NOT NULL \
         ), \
         binned AS ( \
           SELECT \
             CASE \
               WHEN s.col_min = s.col_max THEN 1 \
               WHEN \"{column}\"::FLOAT8 >= s.col_max THEN 20 \
               ELSE FLOOR((\"{column}\"::FLOAT8 - s.col_min) / (s.col_max - s.col_min) * 20)::INT + 1 \
             END AS bucket, \
             s.col_min, s.col_max \
           FROM {schema}.\"{table}\" t, stats s \
           {where_clause} AND \"{column}\" IS NOT NULL \
         ) \
         SELECT bucket, \
           col_min + (bucket - 1) * (col_max - col_min) / 20.0 AS bin_min, \
           col_min + bucket * (col_max - col_min) / 20.0 AS bin_max, \
           COUNT(*) AS count \
         FROM binned GROUP BY bucket, col_min, col_max ORDER BY bucket"
    )
}

// ── Handlers ──────────────────────────────────────────────────────────

/// Get distinct values for a column
#[utoipa::path(
    get,
    path = "/api/data_view/column_values/{table_name}/{column_name}",
    tag = "Data View - Column Analysis",
    params(
        ("table_name" = String, Path, description = "Name of the table"),
        ("column_name" = String, Path, description = "Name of the column"),
        ColumnValuesParams,
    ),
    responses(
        (status = 200, description = "Distinct column values", body = Vec<String>),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state, params), fields(table = %table_name, column = %column_name))]
pub async fn values_handler(
    State(state): State<AppState>,
    Path((table_name, column_name)): Path<(String, String)>,
    Query(params): Query<ColumnValuesParams>,
) -> Result<Json<Vec<String>>, AppError> {
    validate_schema_name(&params.schema)?;
    validate_table_name(&table_name)?;

    let valid_cols_sql = build_valid_columns_query(&params.schema, &table_name);
    let col_rows = state.blocking_query(valid_cols_sql).await?;
    let valid_columns: HashSet<String> = col_rows
        .iter()
        .filter_map(|row| row.get("column_name")?.as_str().map(String::from))
        .collect();

    validate_column_name(&column_name, &valid_columns)?;

    let has_timestamp = table_has_load_timestamp(&state, &params.schema, &table_name).await?;

    let sql = build_column_values_query(
        &params.schema,
        &table_name,
        &column_name,
        params.load_timestamp.as_deref(),
        params.limit,
        has_timestamp,
    );
    let rows = state.blocking_query(sql).await?;

    let values: Vec<String> = rows
        .into_iter()
        .filter_map(|row| row.get("value")?.as_str().map(String::from))
        .collect();

    Ok(Json(values))
}

/// Get statistical summary for a column
#[utoipa::path(
    get,
    path = "/api/data_view/column_stats/{table_name}/{column_name}",
    tag = "Data View - Column Analysis",
    params(
        ("table_name" = String, Path, description = "Name of the table"),
        ("column_name" = String, Path, description = "Name of the column"),
        ColumnStatsParams,
    ),
    responses(
        (status = 200, description = "Column statistics including nulls, cardinality, and numeric stats", body = ColumnStatsResponse),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 404, description = "No data found", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state, params), fields(table = %table_name, column = %column_name))]
pub async fn stats_handler(
    State(state): State<AppState>,
    Path((table_name, column_name)): Path<(String, String)>,
    Query(params): Query<ColumnStatsParams>,
) -> Result<Json<ColumnStatsResponse>, AppError> {
    validate_schema_name(&params.schema)?;
    validate_table_name(&table_name)?;

    let valid_cols_sql = build_valid_columns_query(&params.schema, &table_name);
    let col_rows = state.blocking_query(valid_cols_sql).await?;
    let valid_columns: HashSet<String> = col_rows
        .iter()
        .filter_map(|row| row.get("column_name")?.as_str().map(String::from))
        .collect();

    validate_column_name(&column_name, &valid_columns)?;

    let has_timestamp = table_has_load_timestamp(&state, &params.schema, &table_name).await?;

    // Determine the column's data type.
    let type_sql = build_column_type_query(&params.schema, &table_name, &column_name);
    let type_rows = state.blocking_query(type_sql).await?;
    let data_type = type_rows
        .first()
        .and_then(|row| row.get("data_type")?.as_str().map(String::from))
        .unwrap_or_default();
    let is_numeric = is_numeric_data_type(&data_type);

    // Universal stats query.
    let stats_sql = build_column_stats_query(
        &params.schema,
        &table_name,
        &column_name,
        params.load_timestamp.as_deref(),
        has_timestamp,
    );
    let stats_rows = state.blocking_query(stats_sql).await?;

    let stats_row = stats_rows
        .first()
        .ok_or_else(|| AppError::NotFound("No data found".to_string()))?;

    let total_count = stats_row
        .get("total_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let null_count = stats_row
        .get("null_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let null_pct = stats_row
        .get("null_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let distinct_count = stats_row
        .get("distinct_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cardinality_pct = stats_row
        .get("cardinality_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    // Numeric-only stats (if applicable).
    let (min_val, max_val, mean_val, median_val, stddev_val, p25, p75) = if is_numeric {
        let num_sql = build_numeric_stats_query(
            &params.schema,
            &table_name,
            &column_name,
            params.load_timestamp.as_deref(),
            has_timestamp,
        );
        let num_rows = state.blocking_query(num_sql).await?;

        if let Some(row) = num_rows.first() {
            (
                row.get("min_val").and_then(|v| v.as_f64()),
                row.get("max_val").and_then(|v| v.as_f64()),
                row.get("mean_val").and_then(|v| v.as_f64()),
                row.get("median_val").and_then(|v| v.as_f64()),
                row.get("stddev_val").and_then(|v| v.as_f64()),
                row.get("p25").and_then(|v| v.as_f64()),
                row.get("p75").and_then(|v| v.as_f64()),
            )
        } else {
            (None, None, None, None, None, None, None)
        }
    } else {
        (None, None, None, None, None, None, None)
    };

    Ok(Json(ColumnStatsResponse {
        column_name,
        data_type,
        is_numeric,
        total_count,
        null_count,
        null_pct,
        distinct_count,
        cardinality_pct,
        min_val,
        max_val,
        mean_val,
        median_val,
        stddev_val,
        p25,
        p75,
    }))
}

/// Get value distribution for a column (histogram for numeric, top-N for categorical)
#[utoipa::path(
    get,
    path = "/api/data_view/value_distribution/{table_name}/{column_name}",
    tag = "Data View - Column Analysis",
    params(
        ("table_name" = String, Path, description = "Name of the table"),
        ("column_name" = String, Path, description = "Name of the column"),
        ValueDistributionParams,
    ),
    responses(
        (status = 200, description = "Value distribution (histogram bins or top-N values)", body = ValueDistributionResponse),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state, params), fields(table = %table_name, column = %column_name))]
pub async fn distribution_handler(
    State(state): State<AppState>,
    Path((table_name, column_name)): Path<(String, String)>,
    Query(params): Query<ValueDistributionParams>,
) -> Result<Json<ValueDistributionResponse>, AppError> {
    validate_schema_name(&params.schema)?;
    validate_table_name(&table_name)?;

    let valid_cols_sql = build_valid_columns_query(&params.schema, &table_name);
    let col_rows = state.blocking_query(valid_cols_sql).await?;
    let valid_columns: HashSet<String> = col_rows
        .iter()
        .filter_map(|row| row.get("column_name")?.as_str().map(String::from))
        .collect();

    validate_column_name(&column_name, &valid_columns)?;

    let has_timestamp = table_has_load_timestamp(&state, &params.schema, &table_name).await?;

    // Determine the column's data type.
    let type_sql = build_column_type_query(&params.schema, &table_name, &column_name);
    let type_rows = state.blocking_query(type_sql).await?;
    let data_type = type_rows
        .first()
        .and_then(|row| row.get("data_type")?.as_str().map(String::from))
        .unwrap_or_default();
    let is_numeric = is_numeric_data_type(&data_type);

    let distribution = if is_numeric {
        let sql = build_numeric_distribution_query(
            &params.schema,
            &table_name,
            &column_name,
            params.load_timestamp.as_deref(),
            has_timestamp,
        );
        let rows = state.blocking_query(sql).await?;

        rows.into_iter()
            .filter_map(|row| {
                let bucket = row.get("bucket").and_then(|v| v.as_i64()).map(|v| v as i32);
                let bin_min = row.get("bin_min").and_then(|v| v.as_f64());
                let bin_max = row.get("bin_max").and_then(|v| v.as_f64());
                let count = row.get("count").and_then(|v| v.as_i64())?;
                Some(ValueDistributionItem {
                    value: None,
                    bucket,
                    bin_min,
                    bin_max,
                    count,
                })
            })
            .collect()
    } else {
        let sql = build_categorical_distribution_query(
            &params.schema,
            &table_name,
            &column_name,
            params.load_timestamp.as_deref(),
            params.limit,
            has_timestamp,
        );
        let rows = state.blocking_query(sql).await?;

        rows.into_iter()
            .filter_map(|row| {
                let value = row.get("value").and_then(|v| v.as_str()).map(String::from);
                let count = row.get("count").and_then(|v| v.as_i64())?;
                Some(ValueDistributionItem {
                    value,
                    bucket: None,
                    bin_min: None,
                    bin_max: None,
                    count,
                })
            })
            .collect()
    };

    Ok(Json(ValueDistributionResponse {
        column_name,
        data_type,
        is_numeric,
        distribution,
    }))
}
