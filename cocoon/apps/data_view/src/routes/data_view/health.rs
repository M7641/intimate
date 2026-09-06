//! Schema health dashboard — "what's wrong in this database today?"
//!
//! `GET /schema_health` (heavy) returns one health row per event-sourced table
//! in a schema: how fresh its last load is, how many rows the latest snapshot
//! holds, and how that compares to the previous snapshot (the volume trend).
//!
//! It stays to a single round trip the same way `recent_loads` does — one
//! `UNION ALL` of a cheap `COUNT(*) GROUP BY load_timestamp` per table — and
//! lets Rust group the rows and derive each table's trend.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::validation::validate_schema_name;

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, IntoParams)]
pub struct SchemaQueryParam {
    #[serde(default = "default_schema")]
    pub schema: String,
}

fn default_schema() -> String {
    "stage".to_string()
}

/// Health summary for one event-sourced table.
#[derive(Debug, Serialize, ToSchema, PartialEq)]
pub struct TableHealth {
    pub table_name: String,
    /// ISO timestamp of the latest load, or `null` if the table is empty.
    pub last_loaded: Option<String>,
    /// Number of distinct snapshots loaded.
    pub snapshot_count: i64,
    /// Row count of the latest snapshot (0 when the table is empty).
    pub current_rows: i64,
    /// Row count of the previous snapshot, when there is one.
    pub previous_rows: Option<i64>,
    /// Percentage change from previous to current snapshot, when computable.
    pub row_delta_pct: Option<f64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SchemaHealthResponse {
    pub schema: String,
    pub tables: Vec<TableHealth>,
}

// ── Query builders ────────────────────────────────────────────────────

/// Tables in a schema that carry a `load_timestamp` column.
fn build_load_tables_query(schema: &str) -> String {
    let schema_lower = schema.to_lowercase();
    format!(
        "SELECT table_name FROM information_schema.columns \
         WHERE lower(table_schema) = '{schema_lower}' \
         AND lower(column_name) = 'load_timestamp' \
         ORDER BY table_name"
    )
}

/// One query: per-table row count for every snapshot, all tables UNION-ed.
fn build_health_query(schema: &str, tables: &[String]) -> String {
    let selects: Vec<String> = tables
        .iter()
        .map(|t| {
            let label = t.replace('\'', "''");
            format!(
                "SELECT '{label}' AS table_name, \
                 CAST(load_timestamp AS VARCHAR) AS ts, \
                 COUNT(*) AS rc \
                 FROM {schema}.\"{t}\" GROUP BY load_timestamp"
            )
        })
        .collect();
    selects.join(" UNION ALL ")
}

// ── Health derivation ─────────────────────────────────────────────────

/// Turn one table's per-snapshot counts into a health summary.
///
/// `snapshots` is `(load_timestamp, row_count)`; CAST-to-VARCHAR timestamps
/// sort lexically in chronological order, so the latest two are the trend.
fn build_table_health(table_name: &str, mut snapshots: Vec<(String, i64)>) -> TableHealth {
    snapshots.sort_by(|a, b| b.0.cmp(&a.0)); // newest first

    let snapshot_count = snapshots.len() as i64;
    let last_loaded = snapshots.first().map(|(ts, _)| ts.clone());
    let current_rows = snapshots.first().map(|(_, rc)| *rc).unwrap_or(0);
    let previous_rows = snapshots.get(1).map(|(_, rc)| *rc);

    // Delta is only meaningful when the previous snapshot had rows to grow from.
    let row_delta_pct = match previous_rows {
        Some(prev) if prev > 0 => Some(((current_rows - prev) as f64 / prev as f64) * 100.0),
        _ => None,
    };

    TableHealth {
        table_name: table_name.to_string(),
        last_loaded,
        snapshot_count,
        current_rows,
        previous_rows,
        row_delta_pct,
    }
}

/// Assemble the per-table health, including empty tables (which contribute no
/// rows to the query) so a silently-empty table still surfaces.
fn build_health(
    schema: &str,
    all_tables: &[String],
    grouped: BTreeMap<String, Vec<(String, i64)>>,
) -> SchemaHealthResponse {
    let mut grouped = grouped;
    let tables = all_tables
        .iter()
        .map(|t| build_table_health(t, grouped.remove(t).unwrap_or_default()))
        .collect();
    SchemaHealthResponse {
        schema: schema.to_string(),
        tables,
    }
}

// ── Handler ───────────────────────────────────────────────────────────

/// Schema health: a freshness + volume-trend summary for every table.
#[utoipa::path(
    get,
    path = "/api/data_view/schema_health",
    tag = "Data View - Database",
    params(SchemaQueryParam),
    responses(
        (status = 200, description = "Per-table health summary", body = SchemaHealthResponse),
        (status = 400, description = "Invalid schema name", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<SchemaQueryParam>,
) -> Result<Json<SchemaHealthResponse>, AppError> {
    validate_schema_name(&params.schema)?;

    // Which tables are event-sourced (and safe to interpolate)?
    let tables_sql = build_load_tables_query(&params.schema);
    let table_rows = state.blocking_query(tables_sql).await?;
    let tables: Vec<String> = table_rows
        .into_iter()
        .filter_map(|row| {
            row.get("table_name")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .filter(|t| validate_schema_name(t).is_ok())
        .collect();

    if tables.is_empty() {
        return Ok(Json(SchemaHealthResponse {
            schema: params.schema,
            tables: vec![],
        }));
    }

    // One round trip: every snapshot's row count, all tables at once.
    let sql = build_health_query(&params.schema, &tables);
    let rows = state.blocking_query(sql).await?;

    let mut grouped: BTreeMap<String, Vec<(String, i64)>> = BTreeMap::new();
    for row in rows {
        let table = row.get("table_name").and_then(|v| v.as_str());
        let ts = row.get("ts").and_then(|v| v.as_str());
        let rc = row.get("rc").and_then(|v| v.as_i64());
        if let (Some(table), Some(ts), Some(rc)) = (table, ts, rc) {
            grouped
                .entry(table.to_string())
                .or_default()
                .push((ts.to_string(), rc));
        }
    }

    Ok(Json(build_health(&params.schema, &tables, grouped)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trend_from_two_snapshots() {
        let h = build_table_health(
            "orders",
            vec![
                ("2026-06-10 00:00:00".to_string(), 80),
                ("2026-06-14 00:00:00".to_string(), 100),
            ],
        );
        assert_eq!(h.last_loaded.as_deref(), Some("2026-06-14 00:00:00"));
        assert_eq!(h.current_rows, 100);
        assert_eq!(h.previous_rows, Some(80));
        assert_eq!(h.snapshot_count, 2);
        assert_eq!(h.row_delta_pct, Some(25.0));
    }

    #[test]
    fn single_snapshot_has_no_trend() {
        let h = build_table_health("dim", vec![("2026-06-14 00:00:00".to_string(), 5)]);
        assert_eq!(h.current_rows, 5);
        assert_eq!(h.previous_rows, None);
        assert_eq!(h.row_delta_pct, None);
        assert_eq!(h.snapshot_count, 1);
    }

    #[test]
    fn empty_table_surfaces_with_zeroes() {
        let h = build_table_health("empty", vec![]);
        assert_eq!(h.current_rows, 0);
        assert_eq!(h.last_loaded, None);
        assert_eq!(h.snapshot_count, 0);
    }

    #[test]
    fn delta_skipped_when_previous_is_zero() {
        let h = build_table_health(
            "grew_from_empty",
            vec![
                ("2026-06-10 00:00:00".to_string(), 0),
                ("2026-06-14 00:00:00".to_string(), 50),
            ],
        );
        assert_eq!(h.previous_rows, Some(0));
        assert_eq!(h.row_delta_pct, None); // no division by zero
    }

    #[test]
    fn build_health_includes_empty_tables() {
        let all = vec!["a".to_string(), "b".to_string()];
        let mut grouped: BTreeMap<String, Vec<(String, i64)>> = BTreeMap::new();
        grouped.insert(
            "a".to_string(),
            vec![("2026-06-14 00:00:00".to_string(), 10)],
        );
        // "b" produced no rows — it is empty, but must still appear.
        let resp = build_health("stage", &all, grouped);
        assert_eq!(resp.tables.len(), 2);
        let b = resp.tables.iter().find(|t| t.table_name == "b").unwrap();
        assert_eq!(b.current_rows, 0);
        assert_eq!(b.last_loaded, None);
    }
}
