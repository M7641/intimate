//! `GET /api/query-plan` — the operator-level execution plan
//! for one query id.

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use utoipa::IntoParams;

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::response::WarehouseResponse;

/// `?query_id=` — the Snowflake query id (UUID) whose plan to fetch.
#[derive(Deserialize, IntoParams)]
pub struct QueryPlanParams {
    /// Snowflake query ID (UUID format)
    pub query_id: String,
}

pub fn query(query_id: &str) -> String {
    // step_id is required to disambiguate operators: OPERATOR_ID restarts at 0
    // within each step, so a multi-step plan has several operators sharing an id.
    format!(
        "SELECT
            step_id,
            operator_id,
            parent_operators,
            operator_type,
            operator_statistics,
            execution_time_breakdown,
            operator_attributes
        FROM TABLE(GET_QUERY_OPERATOR_STATS('{query_id}'))
        ORDER BY step_id, operator_id"
    )
}

/// Get query execution plan
#[utoipa::path(get, path = "/api/query-plan", tag = "Snowflake",
    params(QueryPlanParams),
    responses(
        (status = 200, body = WarehouseResponse),
        (status = 400, description = "Invalid query_id format", body = ErrorResponse),
        (status = 500, body = ErrorResponse)
    ))]
#[tracing::instrument(skip_all)]
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<QueryPlanParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    // Validate query_id format (Snowflake query IDs are UUIDs). Reject the empty
    // string explicitly: `"".chars().all(..)` is vacuously true, so without this
    // an empty id would reach Snowflake as GET_QUERY_OPERATOR_STATS('') and 500.
    let valid = !params.query_id.is_empty()
        && params.query_id.len() <= 40
        && params
            .query_id
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-');
    if !valid {
        return Err(AppError::Validation("Invalid query_id format".to_string()));
    }
    let sql = query(&params.query_id);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}
