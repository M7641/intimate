//! SQL for the query-history cache. Every statement is built here so the schema
//! and the read/write shapes live in one place.
//!
//! Window bounds are epoch seconds handed in by the caller and materialised with
//! `TO_TIMESTAMP_LTZ(<n>)`, so no timestamp formatting round-trips are needed.

use super::RESULT_LIMIT;

/// Query text is truncated on the way in, matching the 5000-char cap the live
/// analytics endpoints already use.
const QUERY_TEXT_LIMIT: usize = 5000;

/// The columns copied from `INFORMATION_SCHEMA.QUERY_HISTORY` into the cache, in
/// insert order. The source `SELECT`, the `INSERT` list, and the `VALUES` list
/// are all derived from this one slice so they can never drift apart.
///
/// `query_text` is the one exception: it is truncated in the source `SELECT`
/// (see [`source_select`]), not copied verbatim.
const INGEST_COLUMNS: &[&str] = &[
    "query_id",
    "query_hash",
    "query_parameterized_hash",
    "query_text",
    "query_type",
    "database_name",
    "schema_name",
    "user_name",
    "role_name",
    "warehouse_name",
    "warehouse_size",
    "execution_status",
    "error_code",
    "error_message",
    "start_time",
    "end_time",
    "total_elapsed_time",
    "execution_time",
    "compilation_time",
    "queued_overload_time",
    "queued_provisioning_time",
    "bytes_scanned",
    "rows_produced",
    "credits_used_cloud_services",
    "query_tag",
];

/// `CREATE TABLE IF NOT EXISTS` for the cache — an append-only event log of query
/// executions keyed logically (not enforced) on `query_id`.
///
/// This is snowhouse's owned schema: the app defines it, creates it, and keeps
/// it current. `ingested_at` records when each row landed here, independent of
/// when the query ran.
pub(super) fn create_table(table: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {table} (
            query_id STRING,
            query_hash STRING,
            query_parameterized_hash STRING,
            query_text STRING,
            query_type STRING,
            database_name STRING,
            schema_name STRING,
            user_name STRING,
            role_name STRING,
            warehouse_name STRING,
            warehouse_size STRING,
            execution_status STRING,
            error_code STRING,
            error_message STRING,
            start_time TIMESTAMP_LTZ,
            end_time TIMESTAMP_LTZ,
            total_elapsed_time NUMBER,
            execution_time NUMBER,
            compilation_time NUMBER,
            queued_overload_time NUMBER,
            queued_provisioning_time NUMBER,
            bytes_scanned NUMBER,
            rows_produced NUMBER,
            credits_used_cloud_services FLOAT,
            query_tag STRING,
            ingested_at TIMESTAMP_NTZ DEFAULT CURRENT_TIMESTAMP()
        )"
    )
}

/// `SELECT COUNT(*)` over one window, capped at [`RESULT_LIMIT`] by the table
/// function itself. A result equal to the cap signals saturation.
pub(super) fn count_window(lo: i64, hi: i64) -> String {
    format!(
        "SELECT COUNT(*) AS n FROM {history} WHERE end_time IS NOT NULL",
        history = history_fn(lo, hi)
    )
}

/// Idempotent upsert of one window: insert only the `query_id`s not already
/// present. Overlapping windows and re-runs are therefore safe.
pub(super) fn merge_window(table: &str, lo: i64, hi: i64) -> String {
    let insert_cols = INGEST_COLUMNS.join(", ");
    let values = INGEST_COLUMNS
        .iter()
        .map(|c| format!("s.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "MERGE INTO {table} t
        USING (
            SELECT {select_cols}
            FROM {history}
            WHERE end_time IS NOT NULL
        ) s
        ON t.query_id = s.query_id
        WHEN NOT MATCHED THEN INSERT ({insert_cols}) VALUES ({values})",
        select_cols = source_select(),
        history = history_fn(lo, hi),
    )
}

/// The cache high-water mark as epoch seconds (`NULL` when empty).
pub(super) fn watermark(table: &str) -> String {
    format!("SELECT DATE_PART('epoch_second', MAX(end_time)) AS watermark_epoch FROM {table}")
}

/// Row count, covered time span, and last-ingest timestamp for `cache status`.
pub(super) fn status(table: &str) -> String {
    format!(
        "SELECT
            COUNT(*) AS row_count,
            TO_CHAR(MIN(end_time), 'YYYY-MM-DD HH24:MI:SS') AS earliest,
            TO_CHAR(MAX(end_time), 'YYYY-MM-DD HH24:MI:SS') AS latest,
            TO_CHAR(MAX(ingested_at), 'YYYY-MM-DD HH24:MI:SS') AS last_ingest
        FROM {table}"
    )
}

/// The `INFORMATION_SCHEMA.QUERY_HISTORY` table-function call for a window, keyed
/// on `end_time`. Bounds are epoch seconds materialised as `TIMESTAMP_LTZ`.
fn history_fn(lo: i64, hi: i64) -> String {
    format!(
        "TABLE(snowflake.information_schema.query_history(
            END_TIME_RANGE_START => TO_TIMESTAMP_LTZ({lo}),
            END_TIME_RANGE_END => TO_TIMESTAMP_LTZ({hi}),
            RESULT_LIMIT => {RESULT_LIMIT}
        ))"
    )
}

/// The source projection for the `MERGE`, in [`INGEST_COLUMNS`] order, with
/// `query_text` truncated to [`QUERY_TEXT_LIMIT`].
fn source_select() -> String {
    INGEST_COLUMNS
        .iter()
        .map(|&c| {
            if c == "query_text" {
                format!("SUBSTRING(query_text, 1, {QUERY_TEXT_LIMIT}) AS query_text")
            } else {
                c.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(",\n            ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_insert_and_values_have_equal_arity() {
        let sql = merge_window("db.sch.tbl", 0, 100);
        // Every INSERT column must have a matching VALUES entry, or Snowflake
        // rejects the MERGE. Count `s.<col>` inside the VALUES clause only (the
        // ON clause also references `s.query_id`).
        let values_clause = sql.split("VALUES (").nth(1).expect("VALUES clause");
        let values_count = values_clause.matches("s.").count();
        assert_eq!(INGEST_COLUMNS.len(), values_count);
    }

    #[test]
    fn count_window_uses_the_result_limit_cap() {
        let sql = count_window(10, 20);
        assert!(sql.contains(&RESULT_LIMIT.to_string()));
        assert!(sql.contains("TO_TIMESTAMP_LTZ(10)"));
        assert!(sql.contains("TO_TIMESTAMP_LTZ(20)"));
    }

    #[test]
    fn query_text_is_truncated_in_the_source() {
        let sql = merge_window("t", 0, 1);
        assert!(sql.contains(&format!("SUBSTRING(query_text, 1, {QUERY_TEXT_LIMIT})")));
    }
}
