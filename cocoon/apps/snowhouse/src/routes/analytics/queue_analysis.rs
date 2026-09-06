//! `GET /api/queue-analysis` — query queuing hot spots by
//! warehouse, day and hour.

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
            warehouse_name,
            DATE(start_time) as query_date,
            EXTRACT(hour FROM start_time) as hour_of_day,
            COUNT(*) as query_count,
            ROUND(AVG(queued_provisioning_time) / 1000, 2) as avg_queue_seconds,
            ROUND(MAX(queued_provisioning_time) / 1000, 2) as max_queue_seconds,
            ROUND(AVG(queued_overload_time) / 1000, 2) as avg_overload_queue_seconds,
            SUM(CASE WHEN queued_provisioning_time > 5000 THEN 1 ELSE 0 END) as queries_queued_5s_plus,
            SUM(CASE WHEN queued_provisioning_time > 30000 THEN 1 ELSE 0 END) as queries_queued_30s_plus,
            ROUND(100.0 * SUM(CASE WHEN queued_provisioning_time > 5000 THEN 1 ELSE 0 END) / COUNT(*), 2) as pct_queued_5s_plus
        FROM {source}
        WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
            AND execution_status = 'SUCCESS'
            AND warehouse_name IS NOT NULL
        GROUP BY warehouse_name, DATE(start_time), EXTRACT(hour FROM start_time)
        HAVING COUNT(*) >= 5
        ORDER BY avg_queue_seconds DESC"
    )
}

/// Get query queuing analysis
#[utoipa::path(get, path = "/api/queue-analysis", tag = "Snowflake",
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
