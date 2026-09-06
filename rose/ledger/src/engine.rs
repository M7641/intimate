//! The pluggable-engine seam.
//!
//! Every backend the tool can explain against implements [`Engine`]. Adding
//! Redshift or Snowflake later means writing one more `impl Engine` and
//! pushing it into the registry — nothing else in the app changes. That is
//! the whole point of routing both engines through one trait rather than
//! wiring two bespoke services.

use crate::plan::PlanResult;
use serde::Serialize;
use std::sync::Arc;

/// What family the engine belongs to, so the UI can frame the comparison
/// ("analytical vs transactional") without hard-coding engine names.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineKind {
    /// Row-store, index-driven, OLTP. Postgres.
    Transactional,
    /// Columnar, vectorized, OLAP. DuckDB.
    Analytical,
}

impl EngineKind {
    pub fn label(self) -> &'static str {
        match self {
            EngineKind::Transactional => "transactional (row-store)",
            EngineKind::Analytical => "analytical (columnar, vectorized)",
        }
    }
}

/// A backend we can ask for a query plan.
#[async_trait::async_trait]
pub trait Engine: Send + Sync {
    /// Stable identifier, e.g. `postgres`, `duckdb`. Used as the panel key.
    fn id(&self) -> &'static str;

    /// Display name shown in the UI header.
    fn name(&self) -> &'static str;

    fn kind(&self) -> EngineKind;

    /// Produce a normalized plan for `sql`. When `analyze` is true the engine
    /// actually *executes* the query to gather real cardinalities and timing;
    /// otherwise it only asks the planner for an estimate.
    async fn explain(&self, sql: &str, analyze: bool) -> anyhow::Result<PlanResult>;
}

/// The set of engines the running server exposes, in display order.
#[derive(Clone)]
pub struct Registry {
    engines: Vec<Arc<dyn Engine>>,
}

impl Registry {
    pub fn new(engines: Vec<Arc<dyn Engine>>) -> Self {
        Self { engines }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Engine>> {
        self.engines.iter()
    }
}
