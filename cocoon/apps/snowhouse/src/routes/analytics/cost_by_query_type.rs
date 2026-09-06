//! `GET /api/cost-by-query-type` — cost breakdown by the
//! statement type (SELECT / INSERT / MERGE / …).

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use utoipa::IntoParams;

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::response::WarehouseResponse;
use super::shared::{WAREHOUSE_CREDITS_PER_HOUR, default_30, history_source, validate_days};

/// `?days=` window for this view (defaults to 30, clamped to the 7-day data
/// ceiling by the query itself).
#[derive(Deserialize, IntoParams)]
pub struct CostDaysParams {
    #[serde(default = "default_30")]
    pub days: u32,
}

pub fn query(days: u32) -> String {
    let source = history_source();
    format!(
        "SELECT
            query_type,
            COUNT(*) as query_count,
            COUNT(DISTINCT user_name) as unique_users,
            ROUND(SUM(execution_time) / 1000 / 3600, 2) as total_execution_hours,
            ROUND(AVG(execution_time) / 1000, 2) as avg_execution_seconds,
            ROUND(SUM(bytes_scanned) / (1024 * 1024 * 1024 * 1024), 4) as total_tb_scanned,
            ROUND(SUM(credits_used_cloud_services), 4) as cloud_services_credits,
            ROUND(SUM(
                (execution_time / 1000.0 / 3600.0) * {WAREHOUSE_CREDITS_PER_HOUR}
            ), 4) as estimated_compute_credits
        FROM {source}
        WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
        GROUP BY query_type
        ORDER BY estimated_compute_credits DESC"
    )
}

/// Get cost breakdown by query type
#[utoipa::path(get, path = "/api/cost-by-query-type", tag = "Snowflake",
    params(CostDaysParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<CostDaysParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = query(params.days);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}
