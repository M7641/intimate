//! POST /api/allocation_plan/{run_id}/save — persist the current edited plan
//! as a fresh, immutable snapshot.
//!
//! The request body carries the full row-set from the grid (the displayed,
//! post-edit state). The endpoint:
//!
//! 1. Allocates a fresh `snapshot_id` (and chains `parent_snapshot_id` from the
//!    last save in the room, if any).
//! 2. Bulk-inserts every row into `weekly_allocation_plan_snapshots`.
//! 3. Clears the in-memory live edits — they have now graduated from "draft"
//!    to "persisted".
//! 4. Broadcasts a `Saved` event so other connected clients can drop their
//!    own pending edits and refresh.

use std::collections::BTreeMap;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use common_rs::hash::generate_hash_id;
use common_rs::validation::SAFE_HEX_64;
use serde::Deserialize;
use uuid::Uuid;

use crate::state::{AppState, PlanEvent, SnapshotMeta};
use common_rs::auth::AuthenticatedUser;
use common_rs::error::{ApiError, ApiResult};

use super::PlanRow;

/// Floating-point slack for cap comparisons. Mirrors the frontend tolerance
/// so a row that the UI accepted as exactly-at-capacity (`100.0` against a
/// `100.0` cap) is not rejected here over IEEE-754 dust.
const CAP_TOLERANCE: f64 = 1e-6;

const COLS_PER_ROW: usize = 22;
/// Postgres FE/BE protocol encodes parameter count as an `int16`, hard-capping
/// a single bind message at 32 767 parameters. We keep generous headroom.
const MAX_PARAMS_PER_STMT: usize = 30_000;
/// Estimated bind-payload ceiling per statement. Redshift caps a single
/// statement at 16 MB; with placeholders the SQL text is tiny, so the bind
/// values dominate. 8 MB leaves ample margin and keeps single round-trip
/// latency predictable.
const MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
/// Last-line defence: even if size estimates drift, never emit more than
/// this many rows in one statement. In practice the parameter cap above
/// (≈ 1 360 rows at 22 cols) binds first.
const MAX_ROWS_PER_CHUNK: usize = 5_000;

#[derive(Deserialize)]
pub struct SavePlanRequest {
    pub rows: Vec<PlanRow>,
}

pub async fn save_plan(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Path(run_id): Path<String>,
    Json(body): Json<SavePlanRequest>,
) -> ApiResult<impl IntoResponse> {
    if !SAFE_HEX_64.is_match(&run_id) && run_id.len() > 64 {
        return Err(ApiError::BadRequest(format!(
            "invalid optimiser_run_id: {run_id:?}"
        )));
    }
    if body.rows.is_empty() {
        return Err(ApiError::BadRequest(
            "Cannot save an empty plan — refuse rather than wipe history".to_string(),
        ));
    }

    // Hard caps: refuse to persist a plan that over-allocates any supply or
    // any demand. The frontend already gates each cell against the same
    // rules, but a save can race a peer's commit, an altered client could
    // bypass the UI, or a future regression could weaken the validator —
    // so we re-check server-side as a rampart. All violations are reported
    // at once instead of stopping at the first.
    if let Err(violations) = validate_caps(&body.rows) {
        let header = if violations.len() == 1 {
            "Plan violates 1 cap".to_string()
        } else {
            format!("Plan violates {} caps", violations.len())
        };
        return Err(ApiError::BadRequest(format!(
            "{header}:\n  - {}",
            violations.join("\n  - ")
        )));
    }

    let room = state.plan_room(&run_id);
    let parent_snapshot_id = room
        .last_save
        .read()
        .await
        .as_ref()
        .map(|m| m.snapshot_id.clone());

    let snapshot_id = Uuid::new_v4().simple().to_string();
    let saved_at = Utc::now();
    let schema = state.db.schema();

    insert_snapshot_rows(
        &state,
        &schema,
        &snapshot_id,
        parent_snapshot_id.as_deref(),
        &run_id,
        &body.rows,
        &user.email,
        saved_at,
    )
    .await?;

    let meta = SnapshotMeta {
        snapshot_id: snapshot_id.clone(),
        parent_snapshot_id: parent_snapshot_id.clone(),
        saved_by: user.email.clone(),
        saved_at,
    };

    {
        let mut live = room.live_edits.write().await;
        live.clear();
    }
    {
        let mut last = room.last_save.write().await;
        *last = Some(meta.clone());
    }

    // Best-effort broadcast: ignore if no subscribers (no-one is watching).
    let _ = room.broadcast.send(PlanEvent::Saved(meta.clone()));

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "status": "success",
            "snapshot_id": snapshot_id,
            "parent_snapshot_id": parent_snapshot_id,
            "saved_at": saved_at,
            "row_count": body.rows.len(),
        })),
    ))
}

#[allow(clippy::too_many_arguments)]
async fn insert_snapshot_rows(
    state: &AppState,
    schema: &str,
    snapshot_id: &str,
    parent_snapshot_id: Option<&str>,
    optimiser_run_id: &str,
    rows: &[PlanRow],
    saved_by: &str,
    saved_at: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    // Each Redshift INSERT pays a fixed compile + commit overhead in the
    // hundreds of ms, dwarfing per-row cost. So we pack as many rows as
    // safely possible into each statement, bounded by three caps (see
    // `pack_chunks`): protocol parameter limit, estimated bind payload,
    // and a hard row cap. For typical save sizes this is one statement.

    // Per-row contribution from save-wide constants. The PK hash is 64
    // hex chars; the three f64s, smallint, timestamp, and Postgres
    // protocol framing add roughly 80 bytes.
    let constants_bytes = 64
        + snapshot_id.len()
        + parent_snapshot_id.map_or(0, |s| s.len())
        + optimiser_run_id.len()
        + saved_by.len()
        + 80;

    for chunk in pack_chunks(rows, constants_bytes) {
        let placeholder_chunks: Vec<String> = (0..chunk.len())
            .map(|i| {
                let base = i * COLS_PER_ROW;
                let phs: Vec<String> = (1..=COLS_PER_ROW)
                    .map(|j| format!("${}", base + j))
                    .collect();
                format!("({})", phs.join(", "))
            })
            .collect();

        let sql = format!(
            r#"INSERT INTO {schema}.weekly_allocation_plan_snapshots
                (row_id, snapshot_id, parent_snapshot_id, optimiser_run_id,
                 supply_id, demand_id, mascode, supcode, variety, hocustcode,
                 prodnum, tier, brand, countsize,
                 demand_wgt, demand_qty, supply_wgt, allocated_wgt, allocated_qty,
                 is_preferred, saved_by, saved_at)
               VALUES {}"#,
            placeholder_chunks.join(", ")
        );

        let mut q = sqlx::query(&sql);
        for row in chunk {
            // PK row_id is unique per (snapshot_id, supply_id, demand_id).
            // Distinct from the wire-level row_id, which is just
            // "supply_id:demand_id" and stays stable across snapshots.
            let pk = generate_hash_id(&[
                Some(snapshot_id),
                row.supply_id.as_deref(),
                row.demand_id.as_deref(),
            ]);
            q = q.bind(pk);
            q = q.bind(snapshot_id);
            q = q.bind(parent_snapshot_id);
            q = q.bind(optimiser_run_id);
            q = q.bind(row.supply_id.clone());
            q = q.bind(row.demand_id.clone());
            q = q.bind(row.mascode.clone());
            q = q.bind(row.supcode.clone());
            q = q.bind(row.variety.clone());
            q = q.bind(row.hocustcode.clone());
            q = q.bind(row.prodnum.clone());
            q = q.bind(row.tier.clone());
            q = q.bind(row.brand.clone());
            q = q.bind(row.countsize.clone());
            q = q.bind(row.demand_wgt);
            q = q.bind(row.demand_qty);
            q = q.bind(row.supply_wgt);
            q = q.bind(row.allocated_wgt);
            q = q.bind(row.allocated_qty);
            q = q.bind(row.is_preferred.unwrap_or(0));
            q = q.bind(saved_by);
            q = q.bind(saved_at);
        }
        q.execute(state.db.pool()).await?;
    }
    Ok(())
}

/// Walk the plan twice — once grouping by `supply_id`, once by `demand_id` —
/// summing `allocated_wgt` per group and comparing against the relevant
/// capacity. Returns the full list of violations in deterministic order
/// (BTreeMap iteration sorts the keys), so two saves of the same broken
/// plan emit the same error and tests can assert against it.
///
/// Rows with a `None` grouping key skip that pass entirely (they belong to
/// no group). Rows with a `None` capacity field skip the comparison for
/// that group (we have no cap to enforce). The capacity within a group
/// should be identical across rows by construction; we defensively take the
/// minimum so a single corrupt row can't smuggle a higher cap through.
fn validate_caps(rows: &[PlanRow]) -> Result<(), Vec<String>> {
    /// (capacity, running sum). `f64::INFINITY` for `cap` means "no cap
    /// known yet" — replaced as soon as a row contributes one.
    type Bucket = (f64, f64);

    fn bucket_update(entry: &mut Bucket, cap: Option<f64>, alloc: f64) {
        if let Some(c) = cap
            && c < entry.0
        {
            entry.0 = c;
        }
        entry.1 += alloc;
    }

    let mut by_supply: BTreeMap<&str, Bucket> = BTreeMap::new();
    let mut by_demand: BTreeMap<&str, Bucket> = BTreeMap::new();

    for row in rows {
        let alloc = row.allocated_wgt.unwrap_or(0.0);
        if let Some(id) = row.supply_id.as_deref() {
            let entry = by_supply.entry(id).or_insert((f64::INFINITY, 0.0));
            bucket_update(entry, row.supply_wgt, alloc);
        }
        if let Some(id) = row.demand_id.as_deref() {
            let entry = by_demand.entry(id).or_insert((f64::INFINITY, 0.0));
            bucket_update(entry, row.demand_wgt, alloc);
        }
    }

    let mut violations = Vec::new();
    for (id, (cap, sum)) in &by_supply {
        if cap.is_finite() && *sum > cap + CAP_TOLERANCE {
            violations.push(format!("Supply {id} over-allocated: {sum:.1} > {cap:.1}"));
        }
    }
    for (id, (cap, sum)) in &by_demand {
        if cap.is_finite() && *sum > cap + CAP_TOLERANCE {
            violations.push(format!("Demand {id} over-allocated: {sum:.1} > {cap:.1}"));
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// Approximate bytes a single row contributes to the bind payload.
/// `constants_bytes` covers per-row contributions from fields that are
/// constant across the whole save (snapshot ids, run id, saved_by) plus
/// the row_id hash and fixed-size numeric/timestamp framing.
fn estimate_row_bytes(row: &PlanRow, constants_bytes: usize) -> usize {
    let s = |o: &Option<String>| o.as_ref().map_or(0, |x| x.len());
    constants_bytes
        + s(&row.supply_id)
        + s(&row.demand_id)
        + s(&row.mascode)
        + s(&row.supcode)
        + s(&row.variety)
        + s(&row.hocustcode)
        + s(&row.prodnum)
        + s(&row.tier)
        + s(&row.brand)
        + s(&row.countsize)
}

/// Greedily pack rows into chunks bounded by parameter count, estimated
/// bind-payload bytes, and a hard row cap. Whichever cap binds first
/// closes the chunk. A single oversized row is still emitted in its own
/// chunk rather than dropped — better to let Redshift complain than to
/// silently lose data.
fn pack_chunks(rows: &[PlanRow], constants_bytes: usize) -> Vec<&[PlanRow]> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut bytes = 0usize;

    for (i, row) in rows.iter().enumerate() {
        let row_bytes = estimate_row_bytes(row, constants_bytes);
        let next_len = i - start + 1;
        let would_exceed = bytes + row_bytes > MAX_PAYLOAD_BYTES
            || next_len * COLS_PER_ROW > MAX_PARAMS_PER_STMT
            || next_len > MAX_ROWS_PER_CHUNK;

        if would_exceed && i > start {
            chunks.push(&rows[start..i]);
            start = i;
            bytes = 0;
        }
        bytes += row_bytes;
    }
    if start < rows.len() {
        chunks.push(&rows[start..]);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(
        supply_id: &str,
        demand_id: &str,
        supply_wgt: f64,
        demand_wgt: f64,
        allocated: f64,
    ) -> PlanRow {
        PlanRow {
            row_id: format!("{supply_id}:{demand_id}"),
            supply_id: Some(supply_id.to_string()),
            demand_id: Some(demand_id.to_string()),
            mascode: None,
            supcode: None,
            variety: None,
            hocustcode: None,
            prodnum: None,
            tier: None,
            brand: None,
            countsize: None,
            demand_wgt: Some(demand_wgt),
            demand_qty: None,
            supply_wgt: Some(supply_wgt),
            allocated_wgt: Some(allocated),
            allocated_qty: None,
            is_preferred: None,
        }
    }

    #[test]
    fn accepts_perfectly_balanced_plan() {
        let rows = vec![
            make_row("S1", "D1", 10.0, 10.0, 6.0),
            make_row("S1", "D2", 10.0, 4.0, 4.0),
        ];
        assert!(validate_caps(&rows).is_ok());
    }

    #[test]
    fn accepts_at_capacity_within_tolerance() {
        // Floating drift puts us 1e-9 over the cap — should still pass.
        let rows = vec![make_row("S1", "D1", 10.0, 10.0, 10.0 + 1e-9)];
        assert!(validate_caps(&rows).is_ok());
    }

    #[test]
    fn rejects_supply_over_allocation() {
        let rows = vec![
            make_row("S1", "D1", 10.0, 10.0, 7.0),
            make_row("S1", "D2", 10.0, 10.0, 5.0),
        ];
        let err = validate_caps(&rows).unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("Supply S1"));
        assert!(err[0].contains("12.0"));
        assert!(err[0].contains("10.0"));
    }

    #[test]
    fn rejects_demand_over_allocation_across_growers() {
        // Two growers (different supply_id) feeding the same demand,
        // collectively over the cap.
        let rows = vec![
            make_row("S1", "D1", 100.0, 10.0, 7.0),
            make_row("S2", "D1", 100.0, 10.0, 5.0),
        ];
        let err = validate_caps(&rows).unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("Demand D1"));
    }

    #[test]
    fn reports_all_violations_at_once() {
        let rows = vec![
            // S1 over by 2, D1 over by 2.
            make_row("S1", "D1", 10.0, 10.0, 12.0),
            // S2 fine, D2 over by 5.
            make_row("S2", "D2", 100.0, 10.0, 15.0),
        ];
        let err = validate_caps(&rows).unwrap_err();
        assert_eq!(err.len(), 3);
    }

    /// Typical save sizes pack into one chunk regardless of cap inspection.
    #[test]
    fn pack_chunks_fits_typical_save_in_one_statement() {
        let rows: Vec<PlanRow> = (0..1000)
            .map(|i| make_row(&format!("S{i}"), &format!("D{i}"), 10.0, 10.0, 1.0))
            .collect();
        let chunks = pack_chunks(&rows, 200);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 1000);
    }

    /// At 22 cols/row, 30 000 params caps each chunk at ~1 360 rows.
    #[test]
    fn pack_chunks_splits_on_parameter_cap() {
        let rows: Vec<PlanRow> = (0..3500)
            .map(|i| make_row(&format!("S{i}"), &format!("D{i}"), 10.0, 10.0, 1.0))
            .collect();
        let chunks = pack_chunks(&rows, 200);
        assert!(
            chunks.len() >= 3,
            "expected ≥3 chunks, got {}",
            chunks.len()
        );
        for chunk in &chunks {
            assert!(
                chunk.len() * COLS_PER_ROW <= MAX_PARAMS_PER_STMT,
                "chunk has {} rows × {} cols > param cap",
                chunk.len(),
                COLS_PER_ROW
            );
        }
        let total: usize = chunks.iter().map(|c| c.len()).sum();
        assert_eq!(total, 3500);
    }

    /// Fat rows (long varchar payloads) close chunks earlier on byte budget.
    #[test]
    fn pack_chunks_splits_on_byte_budget() {
        let big = "x".repeat(10_000);
        let mut row = make_row("S1", "D1", 10.0, 10.0, 1.0);
        row.variety = Some(big.clone());
        row.hocustcode = Some(big.clone());
        row.prodnum = Some(big.clone());
        row.brand = Some(big.clone());
        row.countsize = Some(big);
        // ~50 KB per row × 200 rows = 10 MB → must split under 8 MB budget.
        let rows: Vec<PlanRow> = (0..200).map(|_| row.clone()).collect();
        let chunks = pack_chunks(&rows, 200);
        assert!(
            chunks.len() >= 2,
            "fat rows should split, got {}",
            chunks.len()
        );
        for chunk in &chunks {
            let est: usize = chunk.iter().map(|r| estimate_row_bytes(r, 200)).sum();
            assert!(est <= MAX_PAYLOAD_BYTES + estimate_row_bytes(&chunk[0], 200));
        }
    }

    /// A single oversized row is emitted alone rather than silently dropped.
    #[test]
    fn pack_chunks_emits_oversized_row_solo() {
        let huge = "x".repeat(MAX_PAYLOAD_BYTES + 1);
        let mut row = make_row("S1", "D1", 10.0, 10.0, 1.0);
        row.variety = Some(huge);
        let rows = vec![row];
        let chunks = pack_chunks(&rows, 200);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 1);
    }

    #[test]
    fn pack_chunks_handles_empty_input() {
        let chunks = pack_chunks(&[], 200);
        assert!(chunks.is_empty());
    }
}
