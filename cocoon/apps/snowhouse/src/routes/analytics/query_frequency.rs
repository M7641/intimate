//! `GET /api/query-frequency` — query volume by hour of day.

use axum::Json;
use axum::extract::{Query, State};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::response::WarehouseResponse;
use super::shared::{DaysParams, history_source, validate_days};

pub fn query(days: u32) -> String {
    let source = history_source();
    format!(
        "SELECT
            EXTRACT(hour FROM start_time) as hour_of_day,
            COUNT(*) as total_queries,
            COUNT(DISTINCT query_id) as unique_queries,
            COUNT(DISTINCT user_name) as unique_users,
            COUNT(DISTINCT warehouse_name) as warehouses_used,
            ROUND(AVG(execution_time) / 1000, 2) as avg_execution_seconds,
            ROUND(SUM(bytes_scanned) / (1024 * 1024 * 1024 * 1024), 4) as total_tb_scanned,
            ROUND(SUM(credits_used_cloud_services), 4) as cloud_services_credits
        FROM {source}
        WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
        GROUP BY 1
        ORDER BY 1"
    )
}

/// Get query frequency analysis
#[utoipa::path(get, path = "/api/query-frequency", tag = "Snowflake",
    params(DaysParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<DaysParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = query(params.days);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}
