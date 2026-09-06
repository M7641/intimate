//! Persistent query-history cache.
//!
//! # Why this exists
//!
//! Without account-admin privileges, `INFORMATION_SCHEMA.QUERY_HISTORY` is
//! capped at `RESULT_LIMIT = 10000` rows per call and has **no `OFFSET`** — so a
//! busy week cannot be read in one shot, and there is no way to page by index.
//! This module works around that by owning a real table ([`CACHE_TABLE`],
//! `SANDPIT.SNOWHOUSE_QUERY_HISTORY_CACHE`) that snowhouse populates itself and
//! then queries freely, bypassing the 10k ceiling.
//!
//! # How it stays correct
//!
//! - **Adaptive windowing** ([`ingest_range`]): the table function is paged by
//!   *time* (`END_TIME_RANGE_START/END`), never by offset. A window that returns
//!   exactly 10k rows may have silently truncated, so we probe each window's
//!   `COUNT(*)` and, when it saturates, bisect it in time until every sub-window
//!   fits under the cap. This is what guarantees no gaps.
//! - **Idempotent upsert** ([`merge_window`]): rows are folded in with a
//!   `MERGE … WHEN NOT MATCHED` keyed on `query_id`. Overlapping windows and
//!   re-runs are therefore harmless — a `query_id` already present is skipped.
//!   Backfill and refresh share this one primitive.
//!
//! # Scope caveat
//!
//! The cache is only as complete as the connecting role's visibility: it holds
//! exactly the queries `INFORMATION_SCHEMA.QUERY_HISTORY` returns for that role,
//! no more. With admin/`IMPORTED PRIVILEGES`, `ACCOUNT_USAGE.QUERY_HISTORY`
//! would remove both the 10k cap and this whole module.

mod sql;

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use service_kit::AppState;
use service_kit::error::AppError;

use crate::cli::CacheAction;

/// Name of the query-history cache table the app owns.
///
/// Hard-coded rather than configurable: snowhouse always owns exactly one cache
/// table, it lives in the dev `sandpit` schema, and its name never changes. The
/// `SNOWHOUSE_` prefix marks it as this app's table in a shared schema. The
/// database comes from the Snowflake connection, so this stays a `SCHEMA.TABLE`
/// reference and resolves against the session database.
pub(crate) const CACHE_TABLE: &str = "SANDPIT.SNOWHOUSE_QUERY_HISTORY_CACHE";

/// The table function's hard row ceiling. A window whose `COUNT(*)` reaches this
/// is treated as saturated (possibly truncated) and gets bisected.
const RESULT_LIMIT: i64 = 10_000;

/// The `days` a `backfill` may request is clamped to this — `INFORMATION_SCHEMA`
/// keeps ~7 days of history, so a larger window buys nothing.
const MAX_LOOKBACK_DAYS: i64 = 7;

/// The actual floor for a window's *start*, in seconds. The table function
/// rejects a window that begins at (or before) 7 days ago —
/// "Cannot retrieve data from more than 7 days ago" — so the lower bound must
/// stop just inside the wall. 167 hours (6d23h) gives an hour of margin, exactly
/// as the live analytics queries do (`DATEADD(hour, -167, ...)`). Both backfill
/// and the empty-table refresh clamp `lo` to `now - MAX_LOOKBACK_SECONDS`.
const MAX_LOOKBACK_SECONDS: i64 = 167 * 3600;

/// Seconds a refresh re-scans *before* the stored watermark. The `MERGE` dedups,
/// so this overlap only costs a little re-reading — and it rescues queries whose
/// completion straddled the previous run's boundary. See [`refresh`].
const REFRESH_OVERLAP_SECONDS: i64 = 3600;

/// Enables the in-process hourly refresh ticker inside `serve`.
const AUTO_REFRESH_ENV: &str = "SNOWHOUSE_CACHE_AUTO_REFRESH";

/// Overrides the ticker interval (seconds); defaults to one hour.
const REFRESH_INTERVAL_ENV: &str = "SNOWHOUSE_CACHE_REFRESH_SECS";
const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 3600;

/// Optionally spawn a background task that refreshes the cache on an interval,
/// for a self-contained deployment that has no external scheduler.
///
/// On by default (hourly); set [`AUTO_REFRESH_ENV`] to a falsy value to disable.
///
/// ⚠️ Single-writer assumption: with several replicas each ticker writes
/// independently. The `MERGE` keeps that *correct* (dedup on `query_id`), only
/// wasteful. For a multi-replica deployment prefer one external `cache refresh`
/// job over this ticker.
pub fn spawn_auto_refresh(state: AppState) {
    let enabled = std::env::var(AUTO_REFRESH_ENV)
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE"))
        .unwrap_or(true); // Keep on by default, user to turn off if desired.
    if !enabled {
        return;
    }
    let table = CACHE_TABLE;
    let secs = std::env::var(REFRESH_INTERVAL_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_REFRESH_INTERVAL_SECS);

    tracing::info!(table, interval_secs = secs, "Cache auto-refresh enabled");
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(secs));
        // The first tick fires immediately; skip it so startup isn't blocked by a
        // refresh, then run every `secs`.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if let Err(e) = refresh(&state, table).await {
                tracing::error!(error = %e, "Scheduled cache refresh failed");
            }
        }
    });
}

/// Dispatch a `cache` CLI subcommand against a live warehouse connection.
pub async fn run(state: &AppState, action: CacheAction) -> Result<(), AppError> {
    let table = CACHE_TABLE;
    match action {
        CacheAction::Init => init(state, table).await,
        CacheAction::Backfill { days } => backfill(state, table, days).await,
        CacheAction::Refresh => refresh(state, table).await,
        CacheAction::Status => status(state, table).await,
    }
}

/// `cache init` — create the cache table if it does not already exist.
async fn init(state: &AppState, table: &str) -> Result<(), AppError> {
    state.blocking_execute(sql::create_table(table)).await?;
    tracing::info!(table, "Cache table ready");
    Ok(())
}

/// `cache backfill` — populate the last `days` (clamped to the 7-day retention)
/// in one adaptive sweep. Idempotent: re-running only fills gaps.
async fn backfill(state: &AppState, table: &str, days: u32) -> Result<(), AppError> {
    let clamped = (days as i64).clamp(1, MAX_LOOKBACK_DAYS);
    if clamped != days as i64 {
        tracing::warn!(
            requested = days,
            used = clamped,
            "backfill window clamped to information_schema retention"
        );
    }
    let now = now_epoch();
    let lo = clamp_lookback(now, now - clamped * 86_400);
    tracing::debug!(table, days = clamped, "Backfill starting");
    let inserted = ingest_range(state, table, lo, now).await?;
    tracing::info!(table, inserted, "Backfill complete");
    Ok(())
}

/// `cache refresh` — top up with everything since the last cached query.
///
/// The watermark is `MAX(end_time)` (a query is only final once it *ends*). We
/// re-scan from `watermark - REFRESH_OVERLAP_SECONDS` and let the `MERGE` dedup,
/// so boundary-straddling queries are never missed. An empty table degrades to a
/// full backfill window.
async fn refresh(state: &AppState, table: &str) -> Result<(), AppError> {
    let now = now_epoch();
    let lo = match watermark_epoch(state, table).await? {
        Some(watermark) => watermark - REFRESH_OVERLAP_SECONDS,
        None => {
            tracing::info!(
                table,
                "Cache empty — refresh falls back to a full backfill window"
            );
            now - MAX_LOOKBACK_SECONDS
        }
    };
    // Never reach past retention, however old the watermark — and never at the
    // 7-day wall itself, which the table function rejects.
    let lo = clamp_lookback(now, lo);
    tracing::debug!(table, "Refresh starting");
    let inserted = ingest_range(state, table, lo, now).await?;
    tracing::info!(table, inserted, "Refresh complete");
    Ok(())
}

/// `cache status` — print row count, the time span covered, and last ingest.
async fn status(state: &AppState, table: &str) -> Result<(), AppError> {
    let rows = state.blocking_query_uncached(sql::status(table)).await?;
    let row = rows.first();
    let get = |key: &str| -> String {
        row.and_then(|r| r.get(key))
            .map(render_scalar)
            .unwrap_or_else(|| "-".to_string())
    };
    println!("Cache table : {table}");
    println!("Rows        : {}", get("row_count"));
    println!("Earliest    : {}", get("earliest"));
    println!("Latest      : {}", get("latest"));
    println!("Last ingest : {}", get("last_ingest"));
    Ok(())
}

/// Ingest every query whose `end_time` falls in `[lo, hi]` (epoch seconds),
/// subdividing saturated windows in time so none silently truncates.
///
/// Uses an explicit work stack rather than recursion (no boxed async futures).
/// Returns the total rows the warehouse reported inserting.
async fn ingest_range(state: &AppState, table: &str, lo: i64, hi: i64) -> Result<u64, AppError> {
    let mut stack = vec![(lo, hi)];
    let mut total_inserted = 0u64;

    while let Some((lo, hi)) = stack.pop() {
        if hi <= lo {
            continue;
        }
        let count = probe_count(state, lo, hi).await?;

        // Saturated window: bisect it in time until it fits under the cap. The
        // 1-second floor guards against an instant with >10k queries (which no
        // finer split could resolve) — we ingest it and warn instead of looping.
        if count >= RESULT_LIMIT && (hi - lo) > 1 {
            let mid = lo + (hi - lo) / 2;
            stack.push((lo, mid));
            stack.push((mid, hi));
            continue;
        }
        if count >= RESULT_LIMIT {
            tracing::warn!(
                lo,
                hi,
                "window still saturated at 1s granularity — possible truncation"
            );
        }

        let inserted = merge_window(state, table, lo, hi).await?;
        total_inserted += inserted;
        tracing::debug!(lo, hi, count, inserted, "window ingested");
    }
    Ok(total_inserted)
}

/// How many queries the table function reports for a window, capped at
/// [`RESULT_LIMIT`]. A returned value of exactly `RESULT_LIMIT` is the
/// saturation signal that drives bisection. Cache-bypassing: the answer must be
/// live.
async fn probe_count(state: &AppState, lo: i64, hi: i64) -> Result<i64, AppError> {
    let rows = state
        .blocking_query_uncached(sql::count_window(lo, hi))
        .await?;
    Ok(rows
        .first()
        .and_then(|r| r.get("n"))
        .and_then(scalar_i64)
        .unwrap_or(0))
}

/// Upsert one (already-sized) window into the cache, keyed on `query_id`.
async fn merge_window(state: &AppState, table: &str, lo: i64, hi: i64) -> Result<u64, AppError> {
    state
        .blocking_execute(sql::merge_window(table, lo, hi))
        .await
}

/// The cache's high-water mark as epoch seconds: `MAX(end_time)`, or `None` when
/// the table is empty.
async fn watermark_epoch(state: &AppState, table: &str) -> Result<Option<i64>, AppError> {
    let rows = state.blocking_query_uncached(sql::watermark(table)).await?;
    Ok(rows
        .first()
        .and_then(|r| r.get("watermark_epoch"))
        .and_then(scalar_i64))
}

/// Current wall-clock time as epoch seconds — the ceiling of every window.
fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Pull a window's start forward so it never touches the 7-day retention wall.
///
/// `INFORMATION_SCHEMA.QUERY_HISTORY` rejects a window beginning at (or before)
/// 7 days ago, so `requested` is floored at `now - MAX_LOOKBACK_SECONDS`
/// (167h / 6d23h). Pure, so the boundary that has bitten twice is unit-tested.
fn clamp_lookback(now: i64, requested: i64) -> i64 {
    requested.max(now - MAX_LOOKBACK_SECONDS)
}

/// Read a JSON scalar as `i64`, tolerating the connector returning a number as
/// either an integer or a float (Snowflake `FIXED`/`NUMBER` parsing).
fn scalar_i64(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))
}

/// Render a JSON scalar for the `status` printout (numbers/strings as-is, null
/// as a dash).
fn render_scalar(v: &Value) -> String {
    match v {
        Value::Null => "-".to_string(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookback_never_reaches_the_seven_day_wall() {
        let now = 7_000_000_000;
        // A full 7-day request would start AT the wall, which the table function
        // rejects — it must be pulled forward to the 167h floor.
        assert_eq!(
            clamp_lookback(now, now - 7 * 86_400),
            now - MAX_LOOKBACK_SECONDS
        );
        assert!(clamp_lookback(now, now - 7 * 86_400) > now - 7 * 86_400);
    }

    #[test]
    fn lookback_leaves_recent_windows_untouched() {
        let now = 7_000_000_000;
        // Anything already inside the wall passes through unchanged.
        assert_eq!(clamp_lookback(now, now - 3_600), now - 3_600);
        assert_eq!(clamp_lookback(now, now), now);
    }
}
