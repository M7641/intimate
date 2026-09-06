use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast};

use common_rs::cache::TtlCache;
use common_rs::db::Db;
use common_rs::env::EnvManager;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub env: EnvManager,
    pub cascade_cache: Arc<TtlCache<String, serde_json::Value>>,
    pub plan_rooms: Arc<DashMap<String, Arc<PlanRoom>>>,
}

impl AppState {
    pub fn new(db: Db, env: EnvManager) -> Self {
        Self {
            db,
            env,
            cascade_cache: Arc::new(TtlCache::new(
                "weekly_cascade",
                128,
                std::time::Duration::from_secs(600),
            )),
            plan_rooms: Arc::new(DashMap::new()),
        }
    }

    /// Look up the room for an `optimiser_run_id`, lazily creating it on first
    /// access. Rooms live for the lifetime of the process; no eviction yet.
    pub fn plan_room(&self, optimiser_run_id: &str) -> Arc<PlanRoom> {
        if let Some(room) = self.plan_rooms.get(optimiser_run_id) {
            return Arc::clone(&room);
        }
        self.plan_rooms
            .entry(optimiser_run_id.to_string())
            .or_insert_with(|| Arc::new(PlanRoom::new()))
            .clone()
    }
}

/// One in-memory collaborative-editing session for a single `optimiser_run_id`.
///
/// All state here is volatile — it lives only in this process and dies on
/// restart. Persistence happens explicitly via the Save endpoint, which writes
/// a snapshot row-set into `weekly_allocation_plan_snapshots`.
pub struct PlanRoom {
    pub broadcast: broadcast::Sender<PlanEvent>,
    /// Live cell-level edits keyed by `(row_id, column_id)`. Last-write-wins.
    pub live_edits: RwLock<std::collections::HashMap<(String, String), CellEdit>>,
    pub last_save: RwLock<Option<SnapshotMeta>>,
    pub viewers: std::sync::atomic::AtomicU32,
}

impl PlanRoom {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel::<PlanEvent>(1024);
        Self {
            broadcast: tx,
            live_edits: RwLock::new(std::collections::HashMap::new()),
            last_save: RwLock::new(None),
            viewers: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

impl Default for PlanRoom {
    fn default() -> Self {
        Self::new()
    }
}

/// Single cell change. Today only `column_id == "allocated_wgt"` is accepted —
/// the validation lives in the WebSocket handler, not in this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellEdit {
    pub row_id: String,
    pub column_id: String,
    pub value: f64,
    pub edited_by: String,
    /// Server-stamped epoch milliseconds. Authoritative ordering.
    pub server_ts: i64,
    /// Client-supplied identifier so the originating tab can ignore echoes.
    pub origin_client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub snapshot_id: String,
    pub parent_snapshot_id: Option<String>,
    pub saved_by: String,
    pub saved_at: chrono::DateTime<chrono::Utc>,
}

/// Events broadcast inside one room. Kept separate from the WebSocket wire
/// type so a different transport (SSE, etc.) could be added later without
/// touching room internals.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlanEvent {
    Edit(CellEdit),
    Saved(SnapshotMeta),
    Presence { viewers: u32 },
}
