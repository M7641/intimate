//! GET endpoints — load the current plan and (separately) the save history.

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;
use sqlx::FromRow;

use common_rs::db::{BindValue, Params};
use common_rs::validation::SAFE_HEX_64;

use crate::state::{AppState, CellEdit, SnapshotMeta};
use common_rs::auth::AuthenticatedUser;
use common_rs::error::{ApiError, ApiResult};

use super::PlanRow;

#[derive(Serialize)]
pub struct PlanResponse {
    pub optimiser_run_id: String,
    pub source: PlanSource,
    pub rows: Vec<PlanRow>,
    /// Pending live edits already applied on top of `rows`. Sent so a freshly
    /// connecting client sees what its peers are currently typing.
    pub live_edits: Vec<CellEdit>,
    pub last_save: Option<SnapshotMeta>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanSource {
    /// At least one Save has been pressed for this run.
    Snapshot,
    /// No human edits yet; the rows come straight from the optimiser output.
    OptimiserOutput,
}

#[derive(Serialize, FromRow)]
pub struct SnapshotHistoryEntry {
    pub snapshot_id: String,
    pub parent_snapshot_id: Option<String>,
    pub saved_by: String,
    pub saved_at: String,
    pub row_count: i64,
}

fn validate_run_id(run_id: &str) -> Result<(), ApiError> {
    if !SAFE_HEX_64.is_match(run_id) && run_id.len() > 64 {
        return Err(ApiError::BadRequest(format!(
            "invalid optimiser_run_id: {run_id:?}"
        )));
    }
    Ok(())
}

pub async fn get_plan(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(run_id): Path<String>,
) -> ApiResult<Json<PlanResponse>> {
    validate_run_id(&run_id)?;

    let mut params = Params::new();
    params.insert("run_id".to_string(), BindValue::Text(run_id.clone()));

    // Try snapshot first.
    let snapshot_rows: Vec<PlanRow> = state
        .db
        .load_data(include_str!("sql/snapshot_select.sql"), &params)
        .await?;

    let (rows, source) = if snapshot_rows.is_empty() {
        let fallback: Vec<PlanRow> = state
            .db
            .load_data(include_str!("sql/output_select.sql"), &params)
            .await?;
        (fallback, PlanSource::OptimiserOutput)
    } else {
        (snapshot_rows, PlanSource::Snapshot)
    };

    let room = state.plan_room(&run_id);
    let live_edits: Vec<CellEdit> = {
        let guard = room.live_edits.read().await;
        guard.values().cloned().collect()
    };
    let last_save = room.last_save.read().await.clone();

    Ok(Json(PlanResponse {
        optimiser_run_id: run_id,
        source,
        rows,
        live_edits,
        last_save,
    }))
}

pub async fn get_history(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(run_id): Path<String>,
) -> ApiResult<Json<Vec<SnapshotHistoryEntry>>> {
    validate_run_id(&run_id)?;

    let mut params = Params::new();
    params.insert("run_id".to_string(), BindValue::Text(run_id));

    let rows: Vec<SnapshotHistoryEntry> = state
        .db
        .load_data(
            r#"
            SELECT
                snapshot_id,
                MAX(parent_snapshot_id)            AS parent_snapshot_id,
                MAX(saved_by)                      AS saved_by,
                MAX(saved_at)::varchar             AS saved_at,
                COUNT(*)::bigint                   AS row_count
            FROM {schema}.weekly_allocation_plan_snapshots
            WHERE optimiser_run_id = %(run_id)s
            GROUP BY snapshot_id
            ORDER BY MAX(saved_at) DESC
            "#,
            &params,
        )
        .await?;
    Ok(Json(rows))
}
