use std::collections::HashSet;

use axum::Json;
use axum::extract::{Path, Query, State};
use database::Row;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::validation::{
    EXCLUDED_COLUMNS, build_valid_columns_query, escape_sql_value, validate_column_name,
    validate_schema_name, validate_table_name,
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct DataFilter {
    pub column: String,
    /// Filter operator: equals, contains, startsWith, endsWith
    pub operator: String,
    pub value: String,
}

#[derive(Deserialize, IntoParams)]
pub struct DataParams {
    #[serde(default = "default_schema")]
    pub schema: String,
    /// Filter to a specific snapshot timestamp
    pub load_timestamp: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// JSON-encoded array of DataFilter objects
    pub filters: Option<String>,
}

fn default_schema() -> String {
    "stage".to_string()
}

fn default_limit() -> u32 {
    1000
}

/// Maximum rows a client can request in a single data fetch.
const MAX_DATA_LIMIT: u32 = 10_000;

fn build_data_query(
    schema: &str,
    table: &str,
    timestamp: Option<&str>,
    filter_clause: &str,
    limit: u32,
    has_timestamp: bool,
) -> String {
    let timestamp_clause = if !has_timestamp {
        "WHERE 1=1".to_string()
    } else {
        match timestamp {
            Some(ts) => {
                let safe_ts = escape_sql_value(ts);
                format!("WHERE load_timestamp = '{safe_ts}'")
            }
            None => {
                format!(
                    "WHERE load_timestamp = (SELECT MAX(load_timestamp) FROM {schema}.\"{table}\")"
                )
            }
        }
    };

    format!(
        "SELECT * \
         FROM {schema}.\"{table}\" \
         {timestamp_clause} \
         {filter_clause} \
         LIMIT {limit}"
    )
}

/// Translate a list of DataFilters into a SQL AND clause.
///
/// Each filter becomes a case-insensitive comparison on the column cast to VARCHAR.
/// Supported operators: equals, contains, startsWith, endsWith.
/// Returns an empty string if no valid filters are provided.
fn build_filter_clause(filters: &[DataFilter]) -> String {
    let clauses: Vec<String> = filters
        .iter()
        .filter_map(|f| {
            if f.column.is_empty() || f.value.is_empty() {
                return None;
            }
            let safe_value = escape_sql_value(&f.value);
            let expr = match f.operator.as_str() {
                "equals" => {
                    format!(
                        "LOWER(CAST(\"{}\" AS VARCHAR)) = LOWER('{safe_value}')",
                        f.column
                    )
                }
                "contains" => {
                    format!(
                        "LOWER(CAST(\"{}\" AS VARCHAR)) LIKE LOWER('%{safe_value}%')",
                        f.column
                    )
                }
                "startsWith" => {
                    format!(
                        "LOWER(CAST(\"{}\" AS VARCHAR)) LIKE LOWER('{safe_value}%')",
                        f.column
                    )
                }
                "endsWith" => {
                    format!(
                        "LOWER(CAST(\"{}\" AS VARCHAR)) LIKE LOWER('%{safe_value}')",
                        f.column
                    )
                }
                _ => return None,
            };
            Some(expr)
        })
        .collect();

    if clauses.is_empty() {
        String::new()
    } else {
        format!(" AND {}", clauses.join(" AND "))
    }
}

/// Fetch row data from a table with optional filters
#[utoipa::path(
    get,
    path = "/api/data_view/data/{table_name}",
    tag = "Data View - Data",
    params(
        ("table_name" = String, Path, description = "Name of the table"),
        DataParams,
    ),
    responses(
        (status = 200, description = "Table rows as JSON objects", body = Vec<serde_json::Value>),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state, params), fields(table = %table_name))]
pub async fn handler(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Query(params): Query<DataParams>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    validate_schema_name(&params.schema)?;
    validate_table_name(&table_name)?;

    if params.limit > MAX_DATA_LIMIT {
        return Err(AppError::Validation(format!(
            "limit must be <= {MAX_DATA_LIMIT}, got {}",
            params.limit
        )));
    }

    // Parse filters from JSON query param.
    let parsed_filters: Vec<DataFilter> = match &params.filters {
        Some(json_str) => serde_json::from_str(json_str)
            .map_err(|e| AppError::Validation(format!("Invalid filters JSON: {e}")))?,
        None => Vec::new(),
    };

    // Single metadata query: fetch column names and detect load_timestamp in one shot.
    let cols_sql = build_valid_columns_query(&params.schema, &table_name);
    let col_rows = state.blocking_query(cols_sql).await?;
    let valid_columns: HashSet<String> = col_rows
        .iter()
        .filter_map(|row| row.get("column_name")?.as_str().map(String::from))
        .collect();

    let has_timestamp =
        valid_columns.contains("load_timestamp") || valid_columns.contains("LOAD_TIMESTAMP");

    // Validate filter columns against the real schema.
    let filter_clause = if parsed_filters.is_empty() {
        String::new()
    } else {
        for f in &parsed_filters {
            if !f.column.is_empty() && !f.value.is_empty() {
                validate_column_name(&f.column, &valid_columns)?;
            }
        }
        build_filter_clause(&parsed_filters)
    };

    let sql = build_data_query(
        &params.schema,
        &table_name,
        params.load_timestamp.as_deref(),
        &filter_clause,
        params.limit,
        has_timestamp,
    );
    let rows = state.blocking_query(sql).await?;

    // Strip excluded metadata columns (same as Python .drop(..., strict=False)).
    let result: Vec<serde_json::Value> = rows.into_iter().map(strip_excluded_columns).collect();

    Ok(Json(result))
}

/// Remove internal metadata columns from a row before returning to the client.
fn strip_excluded_columns(mut row: Row) -> serde_json::Value {
    for col in EXCLUDED_COLUMNS.iter() {
        row.remove(*col);
    }
    serde_json::Value::Object(row.into_iter().collect())
}
