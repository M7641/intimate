//! `GET /api/repeated-queries` — frequently re-run query
//! shapes (by parameterized hash) and their aggregate cost.

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::response::WarehouseResponse;
use super::shared::{
    WAREHOUSE_CREDITS_PER_HOUR, default_5, default_7, history_source, validate_days,
};

/// How two executions are judged to be "the same" query when grouping.
///
/// Snowflake computes both hashes for us, so the choice is only which column we
/// `GROUP BY` — no similarity logic of our own.
#[derive(Deserialize, ToSchema, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    /// Byte-for-byte identical query text (`query_hash`).
    Exact,
    /// Same parameterized shape, ignoring literal values — catches near hits
    /// like `WHERE id = 1` vs `WHERE id = 2` (`query_parameterized_hash`).
    #[default]
    Fuzzy,
}

impl MatchMode {
    /// The `query_history` column this mode groups executions by.
    fn group_column(self) -> &'static str {
        match self {
            MatchMode::Exact => "query_hash",
            MatchMode::Fuzzy => "query_parameterized_hash",
        }
    }
}

/// `?days=&min_count=&mode=` — window, the minimum executions to count as
/// repeated, and whether near hits are folded together (`mode`).
#[derive(Deserialize, IntoParams)]
pub struct RepeatedParams {
    #[serde(default = "default_7")]
    pub days: u32,
    #[serde(default = "default_5")]
    pub min_count: u32,
    #[serde(default)]
    pub mode: MatchMode,
}

pub fn query(days: u32, min_count: u32, mode: MatchMode) -> String {
    let group_column = mode.group_column();
    let source = history_source();
    format!(
        "SELECT
            {group_column} as query_hash,
            ANY_VALUE(SUBSTRING(query_text, 1, 200)) as query_preview,
            ANY_VALUE(SUBSTRING(query_text, 1, 5000)) as query_text,
            ANY_VALUE(query_type) as query_type,
            COUNT(*) as execution_count,
            COUNT(DISTINCT user_name) as unique_users,
            COUNT(DISTINCT warehouse_name) as warehouses_used,
            ROUND(AVG(execution_time) / 1000, 2) as avg_execution_seconds,
            ROUND(SUM(execution_time) / 1000, 2) as total_execution_seconds,
            ROUND(SUM(bytes_scanned) / (1024 * 1024 * 1024), 4) as total_gb_scanned,
            ROUND(SUM(
                (execution_time / 1000.0 / 3600.0) * {WAREHOUSE_CREDITS_PER_HOUR}
            ), 6) as estimated_total_credits
        FROM {source}
        WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
            AND execution_status = 'SUCCESS'
            AND warehouse_name IS NOT NULL
        GROUP BY {group_column}
        HAVING COUNT(*) >= {min_count}
        ORDER BY estimated_total_credits DESC
        LIMIT 100"
    )
}

/// Get repeated query detection
#[utoipa::path(get, path = "/api/repeated-queries", tag = "Snowflake",
    params(RepeatedParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<RepeatedParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = query(params.days, params.min_count, params.mode);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}
