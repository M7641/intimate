//! Pieces shared across the Snowflake analytics endpoints: request-validation
//! limits, the SQL fragments every credit query builds on, and the common
//! query-parameter structs. Endpoint-specific params and SQL live in each
//! endpoint's own module.

use std::sync::OnceLock;

use serde::Deserialize;
use utoipa::IntoParams;

use service_kit::error::AppError;

/// The fully-qualified query-history cache table every analytics endpoint reads
/// `FROM`, published once at startup by `main`.
///
/// Phase 2 reads history exclusively from this owned table — there is no
/// `information_schema` fallback — so the builders need the table name at
/// SQL-build time. A `OnceLock` lets them reach it without threading it through
/// every `query()` signature.
static HISTORY_SOURCE: OnceLock<String> = OnceLock::new();

/// Publish the resolved cache table name. Called once during server startup,
/// after `main` has verified the table exists. A second call is ignored.
pub fn set_history_source(table: String) {
    let _ = HISTORY_SOURCE.set(table);
}

/// The query-history source every analytics `query()` reads `FROM`.
///
/// In a running server this is the cache table set by [`set_history_source`]; in
/// unit tests (which assert on SQL *shape* only) it falls back to the hard-coded
/// [`CACHE_TABLE`](crate::cache::CACHE_TABLE) so the builders stay callable
/// without a live connection.
pub(super) fn history_source() -> &'static str {
    HISTORY_SOURCE
        .get()
        .map(String::as_str)
        .unwrap_or(crate::cache::CACHE_TABLE)
}

/// Upper bound on the `days` window a caller may request.
pub(super) const MAX_DAYS: u32 = 365;
/// Upper bound on `top_n` / `limit` row counts.
pub(super) const MAX_LIMIT: u32 = 1000;

/// SQL `CASE` mapping a warehouse size to its credits-per-hour rate. Multiply by
/// elapsed hours to estimate a query's compute credits. Shared by the credit
/// queries so the rate table lives in one place.
pub(super) const WAREHOUSE_CREDITS_PER_HOUR: &str = "CASE warehouse_size \
    WHEN 'X-Small' THEN 1 WHEN 'Small' THEN 2 WHEN 'Medium' THEN 4 \
    WHEN 'Large' THEN 8 WHEN 'X-Large' THEN 16 WHEN '2X-Large' THEN 32 \
    WHEN '3X-Large' THEN 64 WHEN '4X-Large' THEN 128 ELSE 1 END";

/// Utility / no-compute statement types excluded from cost/credit views so the
/// numbers reflect real work rather than session chatter:
///   - transaction control: COMMIT, ROLLBACK, BEGIN_TRANSACTION
///   - session config:       ALTER_SESSION, SET, UNSET, USE
///   - metadata / lookups:   SHOW, DESCRIBE
///
/// These carry no meaningful compute and only add noise. (The filter runs in the
/// WHERE clause, after the table function returns, so it improves signal but does
/// not reclaim 10k-row budget — that is what the per-day windowing is for.)
/// A SQL `IN (...)` list, ready to drop after `query_type NOT IN`.
pub(super) const EXCLUDED_QUERY_TYPES: &str = "'COMMIT', 'ROLLBACK', 'BEGIN_TRANSACTION', \
    'ALTER_SESSION', 'SET', 'UNSET', 'USE', 'SHOW', 'DESCRIBE'";

pub(super) fn validate_days(days: u32) -> Result<(), AppError> {
    if days > MAX_DAYS {
        return Err(AppError::Validation(format!(
            "days must be <= {MAX_DAYS}, got {days}"
        )));
    }
    Ok(())
}

pub(super) fn validate_limit(limit: u32) -> Result<(), AppError> {
    if limit > MAX_LIMIT {
        return Err(AppError::Validation(format!(
            "limit must be <= {MAX_LIMIT}, got {limit}"
        )));
    }
    Ok(())
}

pub(super) fn default_5() -> u32 {
    5
}
pub(super) fn default_7() -> u32 {
    7
}
pub(super) fn default_30() -> u32 {
    30
}
pub(super) fn default_50() -> u32 {
    50
}

/// `?days=` — the single-parameter window shared by most endpoints.
#[derive(Deserialize, IntoParams)]
pub(crate) struct DaysParams {
    #[serde(default = "default_7")]
    pub days: u32,
}

/// `?days=&top_n=&search=` — shared by expensive-queries and cost-by-table.
#[derive(Deserialize, IntoParams)]
pub(crate) struct ExpensiveParams {
    #[serde(default = "default_7")]
    pub days: u32,
    #[serde(default = "default_50")]
    pub top_n: u32,
    /// Case-insensitive substring to search for in the query text.
    pub search: Option<String>,
}
