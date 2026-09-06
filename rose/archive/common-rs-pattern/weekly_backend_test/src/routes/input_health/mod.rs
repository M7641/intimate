//! Health metrics for the optimiser's input sources (supply, demand, approval cube).

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use sqlx::FromRow;

use common_rs::db::Params;

use crate::state::AppState;
use common_rs::error::ApiResult;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/input_health/supply_health", get(get_supply_health))
        .route("/input_health/demand_health", get(get_demand_health))
        .route("/input_health/approval_health", get(get_approval_health))
}

#[derive(Serialize, FromRow)]
pub struct HealthRow {
    pub start_date: Option<String>,
    pub session_id: Option<String>,
    pub mascode: Option<String>,
    #[sqlx(default)]
    pub supply_wgt: Option<f64>,
    #[sqlx(default)]
    pub demand_wgt: Option<f64>,
}

#[derive(Serialize, FromRow)]
pub struct ApprovalHealthRow {
    pub latest_timestamp: Option<String>,
    pub total_records: Option<i64>,
}

async fn get_supply_health(State(state): State<AppState>) -> ApiResult<Json<Vec<HealthRow>>> {
    let rows: Vec<HealthRow> = state
        .db
        .load_data(
            r#"
            SELECT
                start_date::varchar AS start_date,
                session_id,
                mascode,
                SUM(wgt) AS supply_wgt
            FROM {schema}.weekly_supply_plan
            WHERE start_date = (SELECT MAX(start_date) FROM {schema}.weekly_supply_plan)
            GROUP BY start_date, session_id, mascode
            "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(rows))
}

async fn get_demand_health(State(state): State<AppState>) -> ApiResult<Json<Vec<HealthRow>>> {
    let rows: Vec<HealthRow> = state
        .db
        .load_data(
            r#"
            SELECT
                start_date::varchar AS start_date,
                session_id,
                mascode,
                SUM(wgt) AS demand_wgt
            FROM {schema}.weekly_demand_plan
            WHERE start_date = (SELECT MAX(start_date) FROM {schema}.weekly_demand_plan)
            GROUP BY start_date, session_id, mascode
            "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(rows))
}

async fn get_approval_health(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<ApprovalHealthRow>>> {
    let rows: Vec<ApprovalHealthRow> = state
        .db
        .load_data(
            r#"
            SELECT
                MAX(load_timestamp)::varchar AS latest_timestamp,
                COUNT(*) AS total_records
            FROM {schema}.daily_approval_cube_uk
            "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(rows))
}
