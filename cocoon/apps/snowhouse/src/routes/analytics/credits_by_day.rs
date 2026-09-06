//! `GET /api/credits-by-day` — estimated compute credits
//! per day (temporal spend view).
//!
//! **CURRENTLY DISABLED — too slow.** On this account the per-day queries against
//! INFORMATION_SCHEMA did not return within the 60 s request budget, so the route
//! and OpenAPI entry are commented out (`mod.rs` / `openapi.rs`) and the UI card
//! is removed. All the code below is kept so it can be switched back on once it
//! runs fast enough.
//!
//! As of Phase 2 (decision 0004) the query reads the owned cache table like every
//! other endpoint, not the table function — so the per-day split and `JoinSet`
//! concurrency below are no longer strictly necessary and could collapse to one
//! `GROUP BY DATE(end_time)`. That simplification is left for whenever this is
//! re-enabled; the historical windowing rationale is kept below for context.
//!
//! A single `information_schema.query_history` call caps at 10 000 rows, so a
//! busy week collapses onto the most recent day. Wrapping seven day-scoped calls
//! in one `UNION ALL` statement fixed the collapse but planned poorly and ran
//! slow in Snowflake. So instead we run ONE small, bounded query per day and
//! concatenate the rows in Rust. Each statement does little work (a single day,
//! ≤10 000 rows, aggregated to one row) and each day keeps its own 10 000-row
//! budget. A single day exceeding 10 000 queries is still capped — the page
//! surfaces that limit.
//!
//! The per-day queries run CONCURRENTLY (a `JoinSet`), so the request time is the
//! slowest single day, not the sum — running them one at a time summed past the
//! 60 s request timeout. Real concurrency is bounded by the connection pool.

use axum::Json;
use axum::extract::{Query, State};
use database::Row;
use tokio::task::JoinSet;

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::response::WarehouseResponse;
use super::shared::{
    DaysParams, EXCLUDED_QUERY_TYPES, WAREHOUSE_CREDITS_PER_HOUR, history_source, validate_days,
};

/// Day offsets to fetch (0 = today, 1 = yesterday, …), oldest first so that
/// concatenating each day's rows yields an ascending-by-day series.
///
/// Clamped to 1..=7: at least one day (so `days = 0` still returns something),
/// at most seven — INFORMATION_SCHEMA only serves 7 days and rejects a window
/// reaching further back, while the API allows `days` up to 365.
fn day_offsets(days: u32) -> Vec<i64> {
    (0..days.clamp(1, 7) as i64).rev().collect()
}

/// SQL for ONE day's credit total, `offset` days ago (0 = today). Returns at most
/// one row (that calendar day), or no row if the day had no qualifying queries.
///
/// The day is bounded by `end_time` via the table function's `END_TIME_RANGE_*`
/// args and bucketed on `DATE(end_time)`, so a query crossing midnight lands in
/// exactly one day. The WHERE only adds a half-open upper cap so a query ending
/// exactly at midnight is not also counted by the next day. Transaction-control
/// and other utility statements ([`EXCLUDED_QUERY_TYPES`]) are filtered out so the
/// chart reflects real compute, not COMMIT/ROLLBACK noise.
fn query_for_day(offset: i64) -> String {
    let start_off = -offset; // midnight of the day, `offset` days ago
    let end_off = 1 - offset; // the following midnight (open upper bound)
    let source = history_source();
    format!(
        "SELECT
            TO_CHAR(DATE(end_time), 'YYYY-MM-DD') AS day,
            COUNT(*) AS query_count,
            ROUND(SUM((execution_time / 1000.0 / 3600.0) * {WAREHOUSE_CREDITS_PER_HOUR}), 6) AS total_credits,
            ROUND(SUM(total_elapsed_time) / 1000.0, 2) AS total_elapsed_seconds
        FROM {source}
        WHERE end_time >= DATEADD(day, {start_off}, DATE_TRUNC('day', CURRENT_TIMESTAMP()))
            AND end_time < DATEADD(day, {end_off}, DATE_TRUNC('day', CURRENT_TIMESTAMP()))
            AND query_type NOT IN ({EXCLUDED_QUERY_TYPES})
        GROUP BY 1"
    )
}

/// Get estimated credits per day (temporal spend view)
#[utoipa::path(get, path = "/api/credits-by-day", tag = "Snowflake",
    params(DaysParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<DaysParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;

    // Fire every day's query at once; the connection pool caps how many actually
    // run in parallel. Each task keeps its offset so we can restore day order
    // afterwards (JoinSet yields in completion order, not spawn order).
    let mut set: JoinSet<(i64, Result<Vec<Row>, AppError>)> = JoinSet::new();
    for offset in day_offsets(params.days) {
        let state = state.clone();
        set.spawn(async move { (offset, state.blocking_query(query_for_day(offset)).await) });
    }

    let mut per_day: Vec<(i64, Vec<Row>)> = Vec::new();
    while let Some(joined) = set.join_next().await {
        let (offset, rows) =
            joined.map_err(|e| AppError::Internal(format!("task join error: {e}")))?;
        per_day.push((offset, rows?));
    }

    // Ascending day = descending offset (offset 0 is today, the largest is oldest).
    per_day.sort_by_key(|(offset, _)| std::cmp::Reverse(*offset));
    let rows: Vec<Row> = per_day
        .into_iter()
        .flat_map(|(_, day_rows)| day_rows)
        .collect();
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_offset_per_day_capped_at_seven() {
        // 0 => a single day (never empty), 7 => seven days, and the API's larger
        // windows clamp to INFORMATION_SCHEMA's 7-day ceiling.
        assert_eq!(day_offsets(0).len(), 1);
        assert_eq!(day_offsets(7).len(), 7);
        assert_eq!(day_offsets(30).len(), 7);
    }

    #[test]
    fn day_offsets_run_oldest_first() {
        // Oldest first so concatenating each day's rows yields ascending days.
        assert_eq!(day_offsets(3), vec![2, 1, 0]);
    }

    #[test]
    fn per_day_query_is_a_single_bounded_scan() {
        let sql = query_for_day(0);
        // Phase 2: reads the owned cache, not the capped table function, and
        // bounds the day at BOTH ends (the lower bound used to come from the
        // table-function's END_TIME_RANGE_START, which is gone).
        assert!(sql.contains(&format!("FROM {}", history_source())));
        assert!(!sql.contains("information_schema.query_history"));
        assert!(sql.contains("end_time >= DATEADD(day"));
        assert!(sql.contains("end_time < DATEADD(day"));
        assert!(!sql.contains("UNION ALL"));
    }

    #[test]
    fn per_day_query_excludes_utility_statements() {
        let sql = query_for_day(0);
        assert!(sql.contains("query_type NOT IN"));
        for t in [
            "'COMMIT'",
            "'ROLLBACK'",
            "'BEGIN_TRANSACTION'",
            "'ALTER_SESSION'",
            "'SET'",
            "'USE'",
            "'SHOW'",
            "'DESCRIBE'",
        ] {
            assert!(sql.contains(t), "expected excluded query type {t}");
        }
    }
}
