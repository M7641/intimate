//! Historical run access: list past runs, fetch their flat allocations, export CSV.

use axum::{
    Json, Router,
    body::Body,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use common_rs::db::{BindValue, Params};
use common_rs::validation::SAFE_HEX_64;

use crate::state::AppState;
use common_rs::error::{ApiError, ApiResult};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/output_runs/output_runs", get(list_output_runs))
        .route("/output_runs/allocations_for_run", get(allocations_for_run))
        .route("/output_runs/download_output", get(download_output))
}

#[derive(Serialize, FromRow)]
pub struct OutputRun {
    pub optimiser_run_id: String,
    pub created_at: String,
}

#[derive(Serialize, FromRow)]
pub struct AllocationOutputRow {
    pub hocustcode: Option<String>,
    pub supcode: Option<String>,
    pub variety: Option<String>,
    pub mascode: Option<String>,
    pub tier: Option<String>,
    pub brand: Option<String>,
    pub countsize: Option<String>,
    pub supply_id: Option<String>,
    pub demand_id: Option<String>,
    pub demand_wgt: Option<f64>,
    pub demand_qty: Option<f64>,
    pub supply_wgt: Option<f64>,
    pub allocated_wgt: Option<f64>,
    pub allocated_qty: Option<f64>,
    pub on_plan: Option<bool>,
}

#[derive(Deserialize)]
pub struct RunQuery {
    pub optimiser_run_id: Option<String>,
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub optimiser_run_id: String,
}

async fn list_output_runs(State(state): State<AppState>) -> ApiResult<Json<Vec<OutputRun>>> {
    let rows: Vec<OutputRun> = state
        .db
        .load_data(
            r#"
            SELECT DISTINCT optimiser_run_id, created_at
            FROM {schema}.weekly_optimiser_output_history
            WHERE optimiser_run_id IS NOT NULL
            ORDER BY created_at DESC
            LIMIT 100
            "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(rows))
}

/// Snapshot-first, optimiser-output fallback — mirrors the read pattern in
/// `allocation_plan::read::get_plan` so a downloaded CSV reflects the latest
/// human-edited plan when one exists, and the raw model output otherwise.
///
/// The snapshot table stores the *full* row-set on every Save (see
/// `allocation_plan::save`), so a hit on `weekly_latest_allocation_plan`
/// wholly supersedes the optimiser output for that run — no per-cell
/// coalescence is needed.
async fn fetch_allocations(
    state: &AppState,
    optimiser_run_id: Option<&str>,
) -> Result<Vec<AllocationOutputRow>, ApiError> {
    if let Some(run_id) = optimiser_run_id {
        if !SAFE_HEX_64.is_match(run_id) && run_id.len() > 64 {
            return Err(ApiError::BadRequest(format!(
                "invalid optimiser_run_id: {run_id:?}"
            )));
        }
        let mut params = Params::new();
        params.insert("run_id".to_string(), BindValue::Text(run_id.to_string()));

        let snapshot_rows: Vec<AllocationOutputRow> = state
            .db
            .load_data(
                r#"
                SELECT
                    hocustcode, supcode, variety, mascode, tier, brand, countsize,
                    supply_id::varchar AS supply_id,
                    demand_id::varchar AS demand_id,
                    demand_wgt, demand_qty, supply_wgt, allocated_wgt, allocated_qty,
                    case when is_preferred = 1 then true else false end as on_plan
                FROM {schema}.weekly_latest_allocation_plan
                WHERE optimiser_run_id = %(run_id)s
                "#,
                &params,
            )
            .await?;

        if !snapshot_rows.is_empty() {
            return Ok(snapshot_rows);
        }

        let rows: Vec<AllocationOutputRow> = state
            .db
            .load_data(
                r#"
                SELECT
                    hocustcode, supcode, variety, mascode, tier, brand, countsize,
                    supply_id::varchar AS supply_id,
                    demand_id::varchar AS demand_id,
                    demand_wgt, demand_qty, supply_wgt, allocated_wgt, allocated_qty,
                    case when is_preferred = 1 then true else false end as on_plan
                FROM {schema}.weekly_optimiser_output_history
                WHERE optimiser_run_id = %(run_id)s
                "#,
                &params,
            )
            .await?;
        Ok(rows)
    } else {
        let rows: Vec<AllocationOutputRow> = state
            .db
            .load_data(
                r#"
                SELECT
                    hocustcode, supcode, variety, mascode, tier, brand, countsize,
                    supply_id::varchar AS supply_id,
                    demand_id::varchar AS demand_id,
                    demand_wgt, demand_qty, supply_wgt, allocated_wgt, allocated_qty,
                    case when is_preferred = 1 then true else false end as on_plan
                FROM {schema}.weekly_optimiser_output
                "#,
                &Params::new(),
            )
            .await?;
        Ok(rows)
    }
}

async fn allocations_for_run(
    State(state): State<AppState>,
    Query(q): Query<RunQuery>,
) -> ApiResult<Json<Vec<AllocationOutputRow>>> {
    let rows = fetch_allocations(&state, q.optimiser_run_id.as_deref()).await?;
    Ok(Json(rows))
}

async fn download_output(
    State(state): State<AppState>,
    Query(q): Query<DownloadQuery>,
) -> ApiResult<Response> {
    let rows = fetch_allocations(&state, Some(&q.optimiser_run_id)).await?;
    if rows.is_empty() {
        return Err(ApiError::NotFound(
            "No allocations found for this run".to_string(),
        ));
    }

    let mut wtr = csv::Writer::from_writer(Vec::<u8>::new());
    wtr.write_record([
        "hocustcode",
        "supcode",
        "variety",
        "mascode",
        "tier",
        "brand",
        "countsize",
        "supply_id",
        "demand_id",
        "demand_wgt",
        "demand_qty",
        "supply_wgt",
        "allocated_wgt",
        "allocated_qty",
        "on_plan",
    ])
    .map_err(|e| ApiError::Internal(format!("csv write: {e}")))?;

    for r in &rows {
        wtr.write_record([
            r.hocustcode.as_deref().unwrap_or(""),
            r.supcode.as_deref().unwrap_or(""),
            r.variety.as_deref().unwrap_or(""),
            r.mascode.as_deref().unwrap_or(""),
            r.tier.as_deref().unwrap_or(""),
            r.brand.as_deref().unwrap_or(""),
            r.countsize.as_deref().unwrap_or(""),
            r.supply_id.as_deref().unwrap_or(""),
            r.demand_id.as_deref().unwrap_or(""),
            &r.demand_wgt.map(|v| v.to_string()).unwrap_or_default(),
            &r.demand_qty.map(|v| v.to_string()).unwrap_or_default(),
            &r.supply_wgt.map(|v| v.to_string()).unwrap_or_default(),
            &r.allocated_wgt.map(|v| v.to_string()).unwrap_or_default(),
            &r.allocated_qty.map(|v| v.to_string()).unwrap_or_default(),
            &r.on_plan.map(|v| v.to_string()).unwrap_or_default(),
        ])
        .map_err(|e| ApiError::Internal(format!("csv write: {e}")))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| ApiError::Internal(format!("csv flush: {e}")))?;

    let filename = format!("weekly_allocation_{}.csv", q.optimiser_run_id);
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/csv"));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(|e| ApiError::Internal(format!("header: {e}")))?,
    );

    Ok((StatusCode::OK, headers, Body::from(bytes)).into_response())
}
