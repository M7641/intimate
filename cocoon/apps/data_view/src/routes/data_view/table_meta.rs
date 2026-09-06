use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::validation::{table_has_load_timestamp, validate_schema_name, validate_table_name};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct ColumnDetail {
    pub column_name: String,
    pub data_type: String,
    pub ordinal_position: i64,
    pub is_nullable: bool,
    // Redshift-specific
    pub encoding: Option<String>,
    pub distkey: Option<bool>,
    pub sortkey: Option<i32>,
    // Snowflake-specific
    pub character_maximum_length: Option<i64>,
    pub numeric_precision: Option<i64>,
    pub numeric_scale: Option<i64>,
    pub column_default: Option<String>,
    pub column_comment: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RedshiftTableStorage {
    pub size_mb: Option<i64>,
    pub tbl_rows: Option<i64>,
    pub unsorted: Option<f64>,
    pub stats_off: Option<f64>,
    pub skew_rows: Option<f64>,
    pub skew_sortkey1: Option<f64>,
    pub encoded: Option<String>,
    pub diststyle: Option<String>,
    pub sortkey1: Option<String>,
    pub sortkey_num: Option<i64>,
    pub pct_used: Option<f64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SnowflakeTableDetails {
    pub row_count: Option<i64>,
    pub bytes: Option<i64>,
    pub retention_time: Option<String>,
    pub created: Option<String>,
    pub last_altered: Option<String>,
    pub auto_clustering_on: Option<bool>,
    pub cluster_by: Option<String>,
    pub is_transient: Option<bool>,
    pub table_type: Option<String>,
    pub table_comment: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TableMetaResponse {
    pub table_name: String,
    pub column_count: usize,
    pub total_rows: Option<i64>,
    pub snapshot_count: Option<i64>,
    pub columns: Vec<ColumnDetail>,
    pub redshift_storage: Option<RedshiftTableStorage>,
    pub snowflake_details: Option<SnowflakeTableDetails>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct TableMetaParams {
    #[serde(default = "default_schema")]
    pub schema: String,
}

fn default_schema() -> String {
    "stage".to_string()
}

// ── Value coercion ────────────────────────────────────────────────────

/// Coerce a JSON value to a bool, accepting native bools, 0/1 numbers, and the
/// "t"/"f"/"true"/"false" strings a driver may emit for Redshift catalog columns.
fn json_to_bool(v: &serde_json::Value) -> Option<bool> {
    match v {
        serde_json::Value::Bool(b) => Some(*b),
        serde_json::Value::Number(n) => n.as_i64().map(|i| i != 0),
        serde_json::Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "t" | "true" | "1" | "yes" => Some(true),
            "f" | "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Coerce a JSON value to an i64, accepting native numbers and numeric strings.
fn json_to_i64(v: &serde_json::Value) -> Option<i64> {
    match v {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        serde_json::Value::Bool(b) => Some(i64::from(*b)),
        _ => None,
    }
}

// ── Query builders ────────────────────────────────────────────────────

/// Extended column metadata from information_schema (universal).
fn build_extended_columns_query(schema: &str, table: &str) -> String {
    let schema_lower = schema.to_lowercase();
    let table_lower = table.to_lowercase();
    format!(
        "SELECT column_name, data_type, ordinal_position, is_nullable, \
         character_maximum_length, numeric_precision, numeric_scale, column_default \
         FROM information_schema.columns \
         WHERE lower(table_schema) = '{schema_lower}' \
         AND lower(table_name) = '{table_lower}' \
         ORDER BY ordinal_position"
    )
}

/// Count distinct load_timestamp snapshots for a table.
fn build_snapshot_count_query(schema: &str, table: &str) -> String {
    format!(
        "SELECT COUNT(DISTINCT load_timestamp) AS snapshot_count \
         FROM {schema}.\"{table}\""
    )
}

/// Row count at the latest snapshot only.
fn build_latest_row_count_query(schema: &str, table: &str) -> String {
    format!(
        "SELECT COUNT(*) AS row_count \
         FROM {schema}.\"{table}\" \
         WHERE load_timestamp = (SELECT MAX(load_timestamp) FROM {schema}.\"{table}\")"
    )
}

/// Redshift table-level storage details from pg_catalog (no superuser needed).
///
/// The sortkey/encoding sub-selects read `pg_attribute` correlated on the outer
/// `pg_class.oid` rather than `pg_table_def`, for the same `search_path` reason
/// as [`build_redshift_column_storage_query`]. `attsortkeyord` is the sort-key
/// position (0 = not a sort key) and `attencodingtype` is 0 when the column is
/// uncompressed (`none`).
fn build_redshift_table_info_query(schema: &str, table: &str) -> String {
    let schema_lower = schema.to_lowercase();
    let table_lower = table.to_lowercase();
    format!(
        "SELECT \
         c.relpages AS size_mb, \
         c.reltuples::BIGINT AS tbl_rows, \
         CASE ci.releffectivediststyle \
           WHEN 0 THEN 'EVEN' \
           WHEN 1 THEN 'KEY' \
           WHEN 8 THEN 'ALL' \
           WHEN 10 THEN 'AUTO(ALL)' \
           WHEN 11 THEN 'AUTO(EVEN)' \
           WHEN 12 THEN 'AUTO(KEY)' \
           ELSE NULL END AS diststyle, \
         (SELECT a.attname FROM pg_attribute a \
          WHERE a.attrelid = c.oid AND a.attsortkeyord = 1 LIMIT 1) AS sortkey1, \
         (SELECT COUNT(*) FROM pg_attribute a \
          WHERE a.attrelid = c.oid AND a.attnum > 0 AND a.attsortkeyord != 0) AS sortkey_num, \
         (SELECT CASE WHEN COUNT(*) > 0 THEN 'Y' ELSE 'N' END FROM pg_attribute a \
          WHERE a.attrelid = c.oid AND a.attnum > 0 AND a.attencodingtype != 0) AS encoded \
         FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         LEFT JOIN pg_class_info ci ON ci.reloid = c.oid \
         WHERE n.nspname = '{schema_lower}' \
         AND c.relname = '{table_lower}'"
    )
}

/// Redshift column-level storage info (encoding, distkey, sortkey).
///
/// Reads the catalog tables directly (`pg_attribute` + `pg_class` +
/// `pg_namespace`) rather than the `pg_table_def` view. `pg_table_def` only
/// returns rows for tables whose schema is on the session `search_path`, so it
/// silently yields nothing for `stage` (the connector stays on `public`) — that
/// was why encoding/distkey/sortkey never came through. The catalog join filters
/// by schema name and is unaffected by `search_path`. `format_encoding` turns the
/// internal encoding-type code into its name (e.g. `az64`, `lzo`, `none`).
fn build_redshift_column_storage_query(schema: &str, table: &str) -> String {
    let schema_lower = schema.to_lowercase();
    let table_lower = table.to_lowercase();
    format!(
        "SELECT a.attname AS \"column\", \
         format_encoding(a.attencodingtype::integer) AS encoding, \
         a.attisdistkey AS distkey, \
         a.attsortkeyord AS sortkey \
         FROM pg_attribute a \
         JOIN pg_class c ON c.oid = a.attrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = '{schema_lower}' \
         AND c.relname = '{table_lower}' \
         AND a.attnum > 0 \
         AND NOT a.attisdropped \
         ORDER BY a.attnum"
    )
}

/// Snowflake extended table info from information_schema.tables.
fn build_snowflake_table_info_query(schema: &str, table: &str) -> String {
    let schema_lower = schema.to_lowercase();
    let table_lower = table.to_lowercase();
    format!(
        "SELECT row_count, bytes, retention_time, created, last_altered, \
         auto_clustering_on, cluster_by, is_transient, table_type, comment AS table_comment \
         FROM information_schema.tables \
         WHERE lower(table_schema) = '{schema_lower}' \
         AND lower(table_name) = '{table_lower}'"
    )
}

/// Snowflake column-level comments from information_schema.columns.
fn build_snowflake_column_comments_query(schema: &str, table: &str) -> String {
    let schema_lower = schema.to_lowercase();
    let table_lower = table.to_lowercase();
    format!(
        "SELECT column_name, comment AS column_comment \
         FROM information_schema.columns \
         WHERE lower(table_schema) = '{schema_lower}' \
         AND lower(table_name) = '{table_lower}'"
    )
}

// ── Handler ───────────────────────────────────────────────────────────

/// Get comprehensive table metadata including columns, storage info, and backend-specific details
#[utoipa::path(
    get,
    path = "/api/data_view/table_meta/{table_name}",
    tag = "Data View - Table Metadata",
    params(
        ("table_name" = String, Path, description = "Name of the table"),
        TableMetaParams,
    ),
    responses(
        (status = 200, description = "Full table metadata", body = TableMetaResponse),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state), fields(table = %table_name))]
pub async fn handler(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Query(params): Query<TableMetaParams>,
) -> Result<Json<TableMetaResponse>, AppError> {
    validate_schema_name(&params.schema)?;
    validate_table_name(&table_name)?;

    let schema = &params.schema;

    // ── Universal queries ─────────────────────────────────────────────

    let has_timestamp = table_has_load_timestamp(&state, schema, &table_name).await?;

    let cols_sql = build_extended_columns_query(schema, &table_name);

    let (col_rows, snapshot_count, total_rows) = if has_timestamp {
        let snap_sql = build_snapshot_count_query(schema, &table_name);
        let rows_sql = build_latest_row_count_query(schema, &table_name);

        let (c, s, r) = tokio::try_join!(
            state.blocking_query(cols_sql),
            state.blocking_query(snap_sql),
            state.blocking_query(rows_sql),
        )?;

        let snap = s
            .first()
            .and_then(|row| row.get("snapshot_count").and_then(|v| v.as_i64()));
        let rows = r
            .first()
            .and_then(|row| row.get("row_count").and_then(|v| v.as_i64()));
        (c, snap, rows)
    } else {
        let count_sql = format!(
            "SELECT COUNT(*) AS row_count FROM {schema}.\"{}\"",
            table_name
        );

        let (c, r) = tokio::try_join!(
            state.blocking_query(cols_sql),
            state.blocking_query(count_sql),
        )?;

        let rows = r
            .first()
            .and_then(|row| row.get("row_count").and_then(|v| v.as_i64()));
        (c, None, rows)
    };

    // Parse columns from information_schema
    let mut columns: Vec<ColumnDetail> = col_rows
        .iter()
        .map(|row| {
            let is_nullable_str = row
                .get("is_nullable")
                .and_then(|v| v.as_str())
                .unwrap_or("NO");
            ColumnDetail {
                column_name: row
                    .get("column_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                data_type: row
                    .get("data_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                ordinal_position: row
                    .get("ordinal_position")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                is_nullable: is_nullable_str.eq_ignore_ascii_case("YES"),
                character_maximum_length: row
                    .get("character_maximum_length")
                    .and_then(|v| v.as_i64()),
                numeric_precision: row.get("numeric_precision").and_then(|v| v.as_i64()),
                numeric_scale: row.get("numeric_scale").and_then(|v| v.as_i64()),
                column_default: row
                    .get("column_default")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                // Backend-specific fields start as None
                encoding: None,
                distkey: None,
                sortkey: None,
                column_comment: None,
            }
        })
        .collect();

    let column_count = columns.len();

    // ── Backend-specific metadata ─────────────────────────────────────

    let mut redshift_storage: Option<RedshiftTableStorage> = None;
    let mut snowflake_details: Option<SnowflakeTableDetails> = None;

    match state.backend() {
        "amazon_redshift" => {
            // SVV_TABLE_INFO
            redshift_storage = match state
                .blocking_query(build_redshift_table_info_query(schema, &table_name))
                .await
            {
                Ok(rows) => rows.first().map(|row| RedshiftTableStorage {
                    size_mb: row.get("size_mb").and_then(|v| v.as_i64()),
                    tbl_rows: row.get("tbl_rows").and_then(|v| v.as_i64()),
                    unsorted: row.get("unsorted").and_then(|v| v.as_f64()),
                    stats_off: row.get("stats_off").and_then(|v| v.as_f64()),
                    skew_rows: row.get("skew_rows").and_then(|v| v.as_f64()),
                    skew_sortkey1: row.get("skew_sortkey1").and_then(|v| v.as_f64()),
                    encoded: row
                        .get("encoded")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    diststyle: row
                        .get("diststyle")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    sortkey1: row
                        .get("sortkey1")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    sortkey_num: row.get("sortkey_num").and_then(|v| v.as_i64()),
                    pct_used: row.get("pct_used").and_then(|v| v.as_f64()),
                }),
                Err(e) => {
                    tracing::warn!(error = %e, "SVV_TABLE_INFO query failed (permissions?)");
                    None
                }
            };

            // Merge encoding/distkey/sortkey into columns from the catalog.
            match state
                .blocking_query(build_redshift_column_storage_query(schema, &table_name))
                .await
            {
                Ok(pg_rows) => {
                    if pg_rows.is_empty() {
                        tracing::warn!(
                            schema = %schema,
                            table = %table_name,
                            "no catalog rows for column storage — distkey/sortkey/encoding will be blank"
                        );
                    } else {
                        tracing::debug!(
                            rows = pg_rows.len(),
                            "redshift column storage rows fetched"
                        );
                    }

                    let pg_map: HashMap<String, &database::Row> = pg_rows
                        .iter()
                        .filter_map(|row| {
                            let col = row.get("column")?.as_str()?.to_string();
                            Some((col, row))
                        })
                        .collect();

                    for col in &mut columns {
                        if let Some(pg_row) = pg_map.get(&col.column_name) {
                            col.encoding = pg_row
                                .get("encoding")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            // The driver may surface these Redshift-specific
                            // catalog columns as strings ("t", "1") rather than
                            // native bool/int, so coerce tolerantly — a strict
                            // `as_bool` / `as_i64` would drop a real value to None
                            // and render "—".
                            col.distkey = pg_row.get("distkey").and_then(json_to_bool);
                            col.sortkey = pg_row
                                .get("sortkey")
                                .and_then(json_to_i64)
                                .map(|v| v as i32);
                        }
                    }
                }
                Err(e) => tracing::warn!(
                    error = %e,
                    "redshift column storage query failed — distkey/sortkey/encoding will be blank"
                ),
            }
        }

        "snowflake" => {
            snowflake_details = match state
                .blocking_query(build_snowflake_table_info_query(schema, &table_name))
                .await
            {
                Ok(rows) => rows.first().map(|row| SnowflakeTableDetails {
                    row_count: row.get("row_count").and_then(|v| v.as_i64()),
                    bytes: row.get("bytes").and_then(|v| v.as_i64()),
                    retention_time: row
                        .get("retention_time")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    created: row
                        .get("created")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    last_altered: row
                        .get("last_altered")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    auto_clustering_on: row.get("auto_clustering_on").and_then(|v| v.as_bool()),
                    cluster_by: row
                        .get("cluster_by")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    is_transient: row.get("is_transient").and_then(|v| v.as_bool()),
                    table_type: row
                        .get("table_type")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    table_comment: row
                        .get("table_comment")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                }),
                Err(e) => {
                    tracing::warn!(error = %e, "Snowflake table info query failed");
                    None
                }
            };

            // Merge column comments
            if let Ok(comment_rows) = state
                .blocking_query(build_snowflake_column_comments_query(schema, &table_name))
                .await
            {
                let comment_map: HashMap<String, String> = comment_rows
                    .iter()
                    .filter_map(|row| {
                        let col = row.get("column_name")?.as_str()?.to_string();
                        let comment = row.get("column_comment")?.as_str()?.to_string();
                        Some((col, comment))
                    })
                    .collect();

                for col in &mut columns {
                    if let Some(comment) = comment_map.get(&col.column_name) {
                        col.column_comment = Some(comment.clone());
                    }
                }
            }
        }

        // DuckDB (or any other) — no extra metadata
        _ => {}
    }

    Ok(Json(TableMetaResponse {
        table_name,
        column_count,
        total_rows,
        snapshot_count,
        columns,
        redshift_storage,
        snowflake_details,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // `pg_table_def` only lists tables whose schema is on the session
    // `search_path`. The connector never sets one, so these queries must read
    // the catalog tables directly or they silently return no rows for `stage`.
    #[test]
    fn column_storage_query_avoids_pg_table_def() {
        let sql = build_redshift_column_storage_query("stage", "my_table");
        assert!(
            !sql.contains("pg_table_def"),
            "must not depend on search_path"
        );
        assert!(sql.contains("pg_attribute"));
        assert!(sql.contains("attisdistkey"));
        assert!(sql.contains("attsortkeyord"));
        assert!(sql.contains("format_encoding"));
        // The merge step keys on a column aliased `"column"`.
        assert!(sql.contains("AS \"column\""));
        assert!(sql.contains("n.nspname = 'stage'"));
        assert!(sql.contains("c.relname = 'my_table'"));
    }

    #[test]
    fn table_info_query_avoids_pg_table_def() {
        let sql = build_redshift_table_info_query("stage", "my_table");
        assert!(
            !sql.contains("pg_table_def"),
            "must not depend on search_path"
        );
        // sortkey/encoding sub-selects now correlate on the outer pg_class.oid.
        assert!(sql.contains("a.attrelid = c.oid"));
        assert!(sql.contains("attsortkeyord"));
        assert!(sql.contains("attencodingtype"));
    }

    // Identifiers are lower-cased before interpolation (catalog names are
    // stored lower-case in Redshift by default).
    #[test]
    fn queries_lowercase_identifiers() {
        let sql = build_redshift_column_storage_query("Stage", "My_Table");
        assert!(sql.contains("'stage'"));
        assert!(sql.contains("'my_table'"));
    }

    // The driver may hand back catalog columns as native types or as strings;
    // coercion must accept both so a real value never renders as "—".
    #[test]
    fn json_to_bool_accepts_native_and_string_forms() {
        use serde_json::json;
        assert_eq!(json_to_bool(&json!(true)), Some(true));
        assert_eq!(json_to_bool(&json!(false)), Some(false));
        assert_eq!(json_to_bool(&json!("t")), Some(true));
        assert_eq!(json_to_bool(&json!("f")), Some(false));
        assert_eq!(json_to_bool(&json!(1)), Some(true));
        assert_eq!(json_to_bool(&json!(0)), Some(false));
        assert_eq!(json_to_bool(&json!(null)), None);
    }

    #[test]
    fn json_to_i64_accepts_native_and_string_forms() {
        use serde_json::json;
        assert_eq!(json_to_i64(&json!(1)), Some(1));
        assert_eq!(json_to_i64(&json!("1")), Some(1));
        assert_eq!(json_to_i64(&json!("0")), Some(0));
        assert_eq!(json_to_i64(&json!(null)), None);
        assert_eq!(json_to_i64(&json!("nope")), None);
    }
}
