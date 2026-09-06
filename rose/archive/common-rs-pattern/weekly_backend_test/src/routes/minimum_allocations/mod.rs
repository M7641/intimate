//! Minimum-allocations CRUD: per-mascode floors below which the optimiser cannot drop.

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use common_rs::db::Params;
use common_rs::validation::SAFE_CODE;

use crate::state::AppState;
use common_rs::error::{ApiError, ApiResult};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/minimum_allocations/",
        get(get_minimum_allocations).post(save_minimum_allocations),
    )
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MinimumAllocation {
    pub mascode: String,
    pub min_allocated_wgt: f64,
}

#[derive(Deserialize)]
pub struct MinimumAllocationsRequest {
    pub allocations: Vec<MinimumAllocation>,
}

#[derive(Serialize)]
pub struct MinimumAllocationsResponse {
    pub allocations: Vec<MinimumAllocation>,
    pub save_timestamp: Option<String>,
}

#[derive(Serialize)]
pub struct MinimumAllocationsSaveResponse {
    pub status: String,
    pub save_timestamp: String,
    pub count: usize,
}

#[derive(FromRow)]
struct AllocationRow {
    save_timestamp: Option<String>,
    mascode: String,
    min_allocated_wgt: f64,
}

async fn get_minimum_allocations(
    State(state): State<AppState>,
) -> ApiResult<Json<MinimumAllocationsResponse>> {
    let rows: Vec<AllocationRow> = state
        .db
        .load_data(
            r#"
            SELECT
                save_timestamp::varchar AS save_timestamp,
                mascode,
                min_allocated_wgt
            FROM {schema}.weekly_minimum_allocations
            WHERE save_timestamp = (
                SELECT MAX(save_timestamp)
                FROM {schema}.weekly_minimum_allocations
            )
            ORDER BY mascode
            "#,
            &Params::new(),
        )
        .await?;

    let save_timestamp = rows.first().and_then(|r| r.save_timestamp.clone());
    let allocations = rows
        .into_iter()
        .map(|r| MinimumAllocation {
            mascode: r.mascode,
            min_allocated_wgt: r.min_allocated_wgt,
        })
        .collect();

    Ok(Json(MinimumAllocationsResponse {
        allocations,
        save_timestamp,
    }))
}

async fn save_minimum_allocations(
    State(state): State<AppState>,
    Json(req): Json<MinimumAllocationsRequest>,
) -> ApiResult<impl IntoResponse> {
    if req.allocations.len() > 10000 {
        return Err(ApiError::BadRequest(
            "max 10000 allocations per request".to_string(),
        ));
    }
    for a in &req.allocations {
        if a.mascode.len() > 16 || !SAFE_CODE.is_match(&a.mascode) {
            return Err(ApiError::BadRequest(format!(
                "invalid mascode: {:?}",
                a.mascode
            )));
        }
        if a.min_allocated_wgt < 0.0 {
            return Err(ApiError::BadRequest(
                "min_allocated_wgt must be >= 0".to_string(),
            ));
        }
    }

    let timestamp = Utc::now();
    let count = req.allocations.len();
    let schema = state.db.schema();
    let mascodes: Vec<String> = req.allocations.iter().map(|a| a.mascode.clone()).collect();
    let wgts: Vec<f64> = req
        .allocations
        .iter()
        .map(|a| a.min_allocated_wgt)
        .collect();

    let sql = format!(
        r#"
            INSERT INTO {schema}.weekly_minimum_allocations
                (save_timestamp, mascode, min_allocated_wgt)
            SELECT $1, mc, w
            FROM UNNEST($2::varchar[], $3::float8[]) AS t(mc, w)
        "#
    );

    sqlx::query(&sql)
        .bind(timestamp)
        .bind(&mascodes)
        .bind(&wgts)
        .execute(state.db.pool())
        .await?;

    tracing::info!(count, ts = %timestamp, "saved minimum allocations");

    Ok((
        StatusCode::OK,
        Json(MinimumAllocationsSaveResponse {
            status: "success".to_string(),
            save_timestamp: timestamp.to_rfc3339(),
            count,
        }),
    ))
}
