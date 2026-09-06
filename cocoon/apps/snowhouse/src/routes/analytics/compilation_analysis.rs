//! `GET /api/compilation-analysis` — compilation-vs-execution
//! time breakdown, surfacing compile-bound query types.

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
            query_type,
            COUNT(*) as query_count,
            ROUND(AVG(compilation_time) / 1000, 2) as avg_compilation_seconds,
            ROUND(AVG(execution_time) / 1000, 2) as avg_execution_seconds,
            ROUND(AVG(total_elapsed_time) / 1000, 2) as avg_total_seconds,
            ROUND(
                CASE
                    WHEN AVG(total_elapsed_time) > 0
                    THEN 100.0 * AVG(compilation_time) / AVG(total_elapsed_time)
                    ELSE 0
                END,
            2) as compilation_pct_of_total,
            ROUND(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY compilation_time) / 1000, 2) as p95_compilation_seconds,
            SUM(CASE WHEN compilation_time > execution_time THEN 1 ELSE 0 END) as queries_compile_bound,
            ROUND(100.0 * SUM(CASE WHEN compilation_time > execution_time THEN 1 ELSE 0 END) / COUNT(*), 2) as pct_compile_bound
        FROM {source}
        WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
            AND execution_status = 'SUCCESS'
            AND warehouse_name IS NOT NULL
        GROUP BY warehouse_name, query_type
        HAVING COUNT(*) >= 10
        ORDER BY avg_compilation_seconds DESC"
    )
}

/// Get compilation time analysis
#[utoipa::path(get, path = "/api/compilation-analysis", tag = "Snowflake",
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
