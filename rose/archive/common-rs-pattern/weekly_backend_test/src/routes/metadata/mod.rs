//! Lookup/metadata endpoints powering frontend filter widgets.

use std::collections::HashSet;

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use sqlx::FromRow;

use common_rs::db::Params;

use crate::state::AppState;
use common_rs::error::ApiResult;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/metadata/mascodes", get(get_available_mascodes))
        .route("/metadata/filter_options", get(get_filter_options))
}

#[derive(Serialize)]
pub struct FilterOptionsResponse {
    pub supcodes: Vec<String>,
    pub hocustcodes: Vec<String>,
    pub mascodes: Vec<String>,
}

#[derive(FromRow)]
struct MascodeRow {
    mascode: String,
}

#[derive(FromRow)]
struct FilterRow {
    supcode: Option<String>,
    hocustcode: Option<String>,
    mascode: Option<String>,
}

async fn get_available_mascodes(State(state): State<AppState>) -> ApiResult<Json<Vec<String>>> {
    let rows: Vec<MascodeRow> = state
        .db
        .load_data(
            "SELECT mascode FROM {schema}.weekly_mascodes",
            &Params::new(),
        )
        .await?;

    Ok(Json(rows.into_iter().map(|r| r.mascode).collect()))
}

async fn get_filter_options(
    State(state): State<AppState>,
) -> ApiResult<Json<FilterOptionsResponse>> {
    let rows: Vec<FilterRow> = state
        .db
        .load_data(
            r#"
            SELECT
                supcode,
                hocustcode,
                mascode
            FROM {schema}.weekly_filter_options
            "#,
            &Params::new(),
        )
        .await?;

    Ok(Json(FilterOptionsResponse {
        supcodes: dedup_sort(rows.iter().filter_map(|r| r.supcode.clone())),
        hocustcodes: dedup_sort(rows.iter().filter_map(|r| r.hocustcode.clone())),
        mascodes: dedup_sort(rows.iter().filter_map(|r| r.mascode.clone())),
    }))
}

fn dedup_sort(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let set: HashSet<String> = values.into_iter().collect();
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    v
}
