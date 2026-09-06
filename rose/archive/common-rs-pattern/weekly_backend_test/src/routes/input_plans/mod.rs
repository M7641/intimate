//! Raw input-plan views: the supply and demand plans the optimiser consumes.

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use sqlx::FromRow;

use common_rs::db::Params;

use crate::state::AppState;
use common_rs::error::ApiResult;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/input_plans/supply_plan_breakdown",
            get(get_supply_plan_breakdown),
        )
        .route(
            "/input_plans/demand_plan_breakdown",
            get(get_demand_plan_breakdown),
        )
}

#[derive(Default, Serialize, FromRow)]
pub struct SupplyPlanRow {
    pub session_id: Option<String>,
    pub prodnum: Option<String>,
    pub wgt: Option<f64>,
    pub dp: Option<String>,
    pub variety: Option<String>,
    pub uomqty: Option<f64>,
    pub supcode: Option<String>,
    pub mascode: Option<String>,
    pub expdate: Option<String>,
    pub start_date: Option<String>,
}

#[derive(Default, Serialize, FromRow)]
pub struct DemandPlanRow {
    pub hocustcode: Option<String>,
    pub deldate: Option<String>,
    pub prodnum: Option<String>,
    pub dept: Option<String>,
    pub brand: Option<String>,
    pub tier: Option<String>,
    pub tier_desc: Option<String>,
    pub session_id: Option<String>,
    pub mascode: Option<String>,
    pub countsize: Option<String>,
    pub wgtouter: Option<f64>,
    pub wgt: Option<f64>,
    pub uomqty: Option<f64>,
    pub start_date: Option<String>,
}

#[derive(Default, Serialize, FromRow)]
pub struct SupplyPlanStats {
    pub total_wgt: Option<f64>,
    pub total_rows: Option<i64>,
    pub mascode_count: Option<i64>,
    pub supcode_count: Option<i64>,
    pub variety_count: Option<i64>,
    pub latest_start_date: Option<String>,
}

#[derive(Default, Serialize, FromRow)]
pub struct DemandPlanStats {
    pub total_wgt: Option<f64>,
    pub total_rows: Option<i64>,
    pub mascode_count: Option<i64>,
    pub hocustcode_count: Option<i64>,
    pub prodnum_count: Option<i64>,
    pub latest_start_date: Option<String>,
}

#[derive(Serialize)]
pub struct SupplyPlanBreakdownResponse {
    pub rows: Vec<SupplyPlanRow>,
    pub stats: SupplyPlanStats,
}

#[derive(Serialize)]
pub struct DemandPlanBreakdownResponse {
    pub rows: Vec<DemandPlanRow>,
    pub stats: DemandPlanStats,
}

async fn get_supply_plan_breakdown(
    State(state): State<AppState>,
) -> ApiResult<Json<SupplyPlanBreakdownResponse>> {
    let rows: Vec<SupplyPlanRow> = state
        .db
        .load_data(
            r#"
            SELECT
                session_id,
                prodnum,
                wgt,
                dp,
                variety,
                uomqty,
                supcode,
                mascode,
                expdate::varchar AS expdate,
                start_date::varchar AS start_date
            FROM {schema}.weekly_supply_plan
            WHERE start_date = (SELECT MAX(start_date) FROM {schema}.weekly_supply_plan)
            ORDER BY mascode, supcode, variety
            "#,
            &Params::new(),
        )
        .await?;

    let stats_rows: Vec<SupplyPlanStats> = state
        .db
        .load_data(
            r#"
            WITH latest AS (
                SELECT MAX(start_date) AS d FROM {schema}.weekly_supply_plan
            )
            SELECT
                COALESCE(SUM(wgt), 0)::float AS total_wgt,
                COUNT(*) AS total_rows,
                COUNT(DISTINCT mascode) AS mascode_count,
                COUNT(DISTINCT supcode) AS supcode_count,
                COUNT(DISTINCT variety) AS variety_count,
                (SELECT d FROM latest)::varchar AS latest_start_date
            FROM {schema}.weekly_supply_plan
            WHERE start_date = (SELECT d FROM latest)
            "#,
            &Params::new(),
        )
        .await?;

    let stats = stats_rows.into_iter().next().unwrap_or_default();
    Ok(Json(SupplyPlanBreakdownResponse { rows, stats }))
}

async fn get_demand_plan_breakdown(
    State(state): State<AppState>,
) -> ApiResult<Json<DemandPlanBreakdownResponse>> {
    let rows: Vec<DemandPlanRow> = state
        .db
        .load_data(
            r#"
            SELECT
                hocustcode,
                deldate::varchar AS deldate,
                prodnum,
                dept,
                brand,
                tier,
                tier_desc,
                session_id,
                mascode,
                countsize,
                wgtouter,
                wgt,
                uomqty,
                start_date::varchar AS start_date
            FROM {schema}.weekly_demand_plan
            WHERE start_date = (SELECT MAX(start_date) FROM {schema}.weekly_demand_plan)
            ORDER BY mascode, hocustcode, prodnum
            "#,
            &Params::new(),
        )
        .await?;

    let stats_rows: Vec<DemandPlanStats> = state
        .db
        .load_data(
            r#"
            WITH latest AS (
                SELECT MAX(start_date) AS d FROM {schema}.weekly_demand_plan
            )
            SELECT
                COALESCE(SUM(wgt), 0)::float AS total_wgt,
                COUNT(*) AS total_rows,
                COUNT(DISTINCT mascode) AS mascode_count,
                COUNT(DISTINCT hocustcode) AS hocustcode_count,
                COUNT(DISTINCT prodnum) AS prodnum_count,
                (SELECT d FROM latest)::varchar AS latest_start_date
            FROM {schema}.weekly_demand_plan
            WHERE start_date = (SELECT d FROM latest)
            "#,
            &Params::new(),
        )
        .await?;

    let stats = stats_rows.into_iter().next().unwrap_or_default();
    Ok(Json(DemandPlanBreakdownResponse { rows, stats }))
}
