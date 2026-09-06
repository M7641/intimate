//! Collaborative editing of the weekly allocation plan.
//!
//! Three layers, kept rigorously separate:
//!
//! 1. **`weekly_optimiser_output_history`** — the immutable model output, never
//!    written to from this module. Read-only fall-back when no human snapshot
//!    exists yet for a given `optimiser_run_id`.
//!
//! 2. **In-memory live state** (`PlanRoom` in `state.rs`) — the volatile
//!    collaborative draft. WebSocket clients fan edits in/out via a
//!    `tokio::sync::broadcast` channel; LWW resolution on `(row_id, column_id)`.
//!
//! 3. **`weekly_allocation_plan_snapshots`** — every Save click writes one
//!    fresh, immutable snapshot under a new `snapshot_id`. The view
//!    `weekly_latest_allocation_plan` exposes the most-recent snapshot per run.
//!
//! The grid identifies rows by a stable string `"{supply_id}:{demand_id}"`;
//! that identifier is used over the WebSocket wire and is recomputed on every
//! load, so it survives snapshot/optimiser refreshes.

use axum::{Router, routing::get};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::state::AppState;

mod read;
mod save;
mod ws;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/allocation_plan/{run_id}", get(read::get_plan))
        .route(
            "/allocation_plan/{run_id}/save",
            axum::routing::post(save::save_plan),
        )
        .route("/allocation_plan/{run_id}/history", get(read::get_history))
        .route("/allocation_plan/{run_id}/ws", get(ws::ws_upgrade))
}

/// One row of an allocation plan as the frontend sees it. Identical shape
/// whether it came from the snapshot table or the optimiser-output fallback.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PlanRow {
    /// Stable wire identifier: `"{supply_id}:{demand_id}"`. Computed in SQL
    /// (see `sql/snapshot_select.sql` and `sql/output_select.sql`) so the same
    /// logical row hashes the same regardless of source.
    pub row_id: String,
    pub supply_id: Option<String>,
    pub demand_id: Option<String>,
    pub mascode: Option<String>,
    pub supcode: Option<String>,
    pub variety: Option<String>,
    pub hocustcode: Option<String>,
    pub prodnum: Option<String>,
    pub tier: Option<String>,
    pub brand: Option<String>,
    pub countsize: Option<String>,
    pub demand_wgt: Option<f64>,
    /// Demand expressed in pack count. Read-only display field surfaced in
    /// the plan editor table; not yet enforced by the cap validator.
    pub demand_qty: Option<f64>,
    pub supply_wgt: Option<f64>,
    pub allocated_wgt: Option<f64>,
    /// Allocation in pack count. Round-tripped on save so a snapshot keeps
    /// the optimiser's pack-level decision; cap checks remain weight-based.
    pub allocated_qty: Option<f64>,
    pub is_preferred: Option<i16>,
}
