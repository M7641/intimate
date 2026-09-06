//! `GET /api/failed-queries` — failed queries grouped by
//! error and origin.

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
            error_code,
            error_message,
            query_type,
            user_name,
            warehouse_name,
            COUNT(*) as failure_count,
            MIN(start_time)::VARCHAR as first_failure,
            MAX(start_time)::VARCHAR as last_failure
        FROM {source}
        WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
            AND execution_status != 'SUCCESS'
        GROUP BY error_code, error_message, query_type, user_name, warehouse_name
        ORDER BY failure_count DESC
        LIMIT 200"
    )
}

/// Get failed query analysis
#[utoipa::path(get, path = "/api/failed-queries", tag = "Snowflake",
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
