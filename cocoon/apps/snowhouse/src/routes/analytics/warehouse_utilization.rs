//! `GET /api/warehouse-utilization` — per-warehouse
//! utilization metrics and sizing recommendations.

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
            -- One row per warehouse: warehouse_size is NULL for queries that used
            -- no warehouse compute (metadata, result-cache hits), which would
            -- otherwise split each warehouse into a NULL-size and a sized group.
            -- MODE() ignores NULLs and returns the warehouse's typical size.
            MODE(warehouse_size) as warehouse_size,
            COUNT(*) as query_count,
            COUNT(DISTINCT user_name) as unique_users,
            ROUND(AVG(execution_time) / 1000, 2) as avg_execution_seconds,
            ROUND(PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY execution_time) / 1000, 2) as p50_execution_seconds,
            ROUND(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time) / 1000, 2) as p95_execution_seconds,
            ROUND(PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY execution_time) / 1000, 2) as p99_execution_seconds,
            ROUND(AVG(queued_provisioning_time) / 1000, 2) as avg_queue_seconds,
            ROUND(SUM(bytes_scanned) / (1024 * 1024 * 1024 * 1024), 4) as total_tb_scanned,
            CASE
                WHEN AVG(queued_provisioning_time) > 10000 THEN 'Consider larger warehouse or multi-cluster'
                WHEN PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time) < 1000 AND MODE(warehouse_size) IN ('Large', 'X-Large', '2X-Large', '3X-Large', '4X-Large') THEN 'Consider smaller warehouse'
                ELSE 'Appropriately sized'
            END as sizing_recommendation
        FROM {source}
        WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
            AND execution_status = 'SUCCESS'
            AND warehouse_name IS NOT NULL
        GROUP BY warehouse_name
        ORDER BY query_count DESC"
    )
}

/// Get warehouse utilization metrics
#[utoipa::path(get, path = "/api/warehouse-utilization", tag = "Snowflake",
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
