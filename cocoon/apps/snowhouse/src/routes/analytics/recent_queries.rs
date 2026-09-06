//! `GET /api/recent-queries` — the latest data-plane queries
//! (SELECT / INSERT / CTAS / MERGE / UPDATE / DELETE).

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use utoipa::IntoParams;

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::response::WarehouseResponse;
use super::shared::{default_7, default_50, history_source, validate_days, validate_limit};

/// `?days=&limit=` — window plus the maximum rows to return.
#[derive(Deserialize, IntoParams)]
pub struct RecentQueriesParams {
    #[serde(default = "default_7")]
    pub days: u32,
    #[serde(default = "default_50")]
    pub limit: u32,
}

pub fn query(days: u32, limit: u32) -> String {
    let source = history_source();
    format!(
        "SELECT
            query_id,
            query_type,
            SUBSTRING(query_text, 1, 5000) as query_text,
            user_name,
            warehouse_name,
            warehouse_size,
            execution_status,
            ROUND(execution_time / 1000, 2) as execution_seconds,
            ROUND(total_elapsed_time / 1000, 2) as total_elapsed_seconds,
            ROUND(bytes_scanned / (1024 * 1024), 2) as mb_scanned,
            rows_produced,
            ROUND(compilation_time / 1000, 2) as compilation_seconds,
            TO_CHAR(start_time, 'YYYY-MM-DD\"T\"HH24:MI:SSTZH:TZM') as start_time,
            TO_CHAR(end_time, 'YYYY-MM-DD\"T\"HH24:MI:SSTZH:TZM') as end_time
        FROM {source}
        WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
            AND query_type IN ('SELECT', 'INSERT', 'CREATE_TABLE_AS_SELECT', 'MERGE', 'UPDATE', 'DELETE')
        ORDER BY start_time DESC
        LIMIT {limit}"
    )
}

/// Get recent query history
#[utoipa::path(get, path = "/api/recent-queries", tag = "Snowflake",
    params(RecentQueriesParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<RecentQueriesParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    validate_limit(params.limit)?;
    let sql = query(params.days, params.limit);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}
