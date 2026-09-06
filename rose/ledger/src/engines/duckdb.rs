//! The analytical engine: embedded DuckDB.
//!
//! DuckDB is columnar and vectorized, so its plan speaks in operator
//! *cardinality* and (under ANALYZE) per-operator *timing* — there is no
//! Postgres-style cost number. We read the plan via `EXPLAIN (FORMAT json)`,
//! which the planner produces without executing, and `EXPLAIN ANALYZE
//! (FORMAT json)`, which runs the query and reports real numbers.

use crate::engine::{Engine, EngineKind};
use crate::plan::{NoteLevel, PlanNode, PlanResult};
use anyhow::{anyhow, Context};
use duckdb::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};

pub struct DuckDbEngine {
    /// The DuckDB crate is blocking and its `Connection` is not `Sync`, so we
    /// guard one connection with a `Mutex` and touch it only inside
    /// `spawn_blocking`. Fine for a single-user prototype.
    conn: Arc<Mutex<Connection>>,
}

impl DuckDbEngine {
    /// Open an in-memory database and run the seed script so DuckDB holds the
    /// same schema and data as the Postgres container. Both engines must see
    /// identical data for a plan comparison to mean anything.
    pub fn in_memory(seed_sql: &str) -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory().context("open DuckDB in-memory")?;
        conn.execute_batch(seed_sql).context("seed DuckDB")?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }
}

#[async_trait::async_trait]
impl Engine for DuckDbEngine {
    fn id(&self) -> &'static str {
        "duckdb"
    }
    fn name(&self) -> &'static str {
        "DuckDB"
    }
    fn kind(&self) -> EngineKind {
        EngineKind::Analytical
    }

    async fn explain(&self, sql: &str, analyze: bool) -> anyhow::Result<PlanResult> {
        let conn = Arc::clone(&self.conn);
        let sql = sql.to_owned();

        // Blocking DuckDB work off the async runtime's worker threads.
        let raw_json = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            // DuckDB takes all EXPLAIN options inside one parenthesis; ANALYZE
            // is an option there, not a leading keyword.
            let opts = if analyze { "ANALYZE, FORMAT json" } else { "FORMAT json" };
            let stmt_sql = format!("EXPLAIN ({opts}) {sql}");
            // Recover from a poisoned lock: a prior panic must not brick the
            // engine for every later request.
            let conn = conn.lock().unwrap_or_else(|e| e.into_inner());
            let mut stmt = conn.prepare(&stmt_sql).context("prepare EXPLAIN")?;
            let mut rows = stmt.query([]).context("run EXPLAIN")?;
            let row = rows
                .next()
                .context("read EXPLAIN row")?
                .ok_or_else(|| anyhow!("EXPLAIN returned no rows"))?;

            // EXPLAIN yields (explain_key, explain_value) columns; the JSON is
            // in one of them. `column_count()` panics before execution in the
            // duckdb crate, so we just probe a few indices and take whichever
            // column parses as JSON. Out-of-range indices return `Err`, which
            // we skip.
            for idx in 0..8 {
                if let Ok(s) = row.get::<usize, String>(idx) {
                    if s.trim_start().starts_with(['{', '[']) {
                        return Ok(s);
                    }
                }
            }
            Err(anyhow!("no JSON column in EXPLAIN output"))
        })
        .await
        .context("DuckDB blocking task panicked")??;

        let value: Value = serde_json::from_str(&raw_json).context("parse DuckDB plan JSON")?;
        let root = parse_node(pick_root(&value));
        let pretty = serde_json::to_string_pretty(&value).unwrap_or(raw_json);

        let mut result = PlanResult::finalize(root, pretty);

        // Engine-specific framing: in a columnar engine a full scan is the
        // expected path, not a warning — the opposite of the Postgres story.
        result = result.with_note(
            NoteLevel::Info,
            None,
            "DuckDB is columnar and vectorized: a full table (SEQ_SCAN) read is the \
             normal, fast path here — unlike a row-store, it does not imply a missing index.",
        );
        if let Some(id) = hot_id(&result) {
            result = result.with_note(
                NoteLevel::Info,
                Some(id),
                "Bottleneck operator by estimated cardinality (or measured time when analyzed).",
            );
        }
        Ok(result)
    }
}

fn hot_id(result: &PlanResult) -> Option<usize> {
    fn find(n: &PlanNode) -> Option<usize> {
        if n.hot {
            return Some(n.id);
        }
        n.children.iter().find_map(find)
    }
    find(&result.root)
}

/// DuckDB wraps the physical plan in a query-root object that carries no
/// operator of its own. If we see such a wrapper with a single child, descend
/// into it so the tree starts at the real root operator.
fn pick_root(value: &Value) -> &Value {
    let mut node = match value {
        Value::Array(items) => items.first().unwrap_or(value),
        other => other,
    };
    // Peel off query-root wrappers: a node with no real operator (plain
    // EXPLAIN) or the synthetic `EXPLAIN_ANALYZE` node (EXPLAIN ANALYZE). These
    // can nest, so descend while the current node is a single-child wrapper.
    loop {
        let is_wrapper = match operator_name(node) {
            None => true,
            Some(name) => name == "EXPLAIN_ANALYZE",
        };
        if !is_wrapper {
            return node;
        }
        match node.get("children").and_then(Value::as_array) {
            Some(children) if children.len() == 1 => node = &children[0],
            _ => return node,
        }
    }
}

fn parse_node(value: &Value) -> PlanNode {
    let mut node = PlanNode::new(operator_name(value).unwrap_or_else(|| "OPERATOR".to_string()));

    let extra = value.get("extra_info");
    node.detail = detail_from_extra(extra);
    node.est_rows = extra.and_then(estimated_cardinality);

    // ANALYZE fills these; a plain EXPLAIN leaves them absent.
    node.actual_rows = value.get("operator_cardinality").and_then(Value::as_f64);
    node.actual_ms = value
        .get("operator_timing")
        .and_then(Value::as_f64)
        .map(|secs| secs * 1000.0);

    if let Some(children) = value.get("children").and_then(Value::as_array) {
        node.children = children.iter().map(parse_node).collect();
    }
    node
}

fn operator_name(value: &Value) -> Option<String> {
    value
        .get("operator_type")
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// `extra_info` is an object in current DuckDB (`{"Table": "...", ...}`) but was
/// an array of strings in older versions. Handle both, and surface only the
/// one or two keys that make a useful one-line qualifier.
fn detail_from_extra(extra: Option<&Value>) -> Option<String> {
    let obj = extra?.as_object();
    if let Some(map) = obj {
        for key in ["Table", "Text", "Function", "Conditions", "Join Type", "Projections"] {
            if let Some(v) = map.get(key).and_then(Value::as_str) {
                if !v.is_empty() {
                    return Some(format!("{key}: {v}"));
                }
            }
        }
        return None;
    }
    if let Some(arr) = extra.and_then(Value::as_array) {
        let joined: Vec<&str> = arr.iter().filter_map(Value::as_str).take(2).collect();
        if !joined.is_empty() {
            return Some(joined.join(" · "));
        }
    }
    None
}

fn estimated_cardinality(extra: &Value) -> Option<f64> {
    let map = extra.as_object()?;
    for key in ["Estimated Cardinality", "Cardinality", "Estimated Rows"] {
        if let Some(v) = map.get(key) {
            // DuckDB stores these as strings; parse leniently.
            if let Some(s) = v.as_str() {
                if let Ok(n) = s.replace(',', "").parse::<f64>() {
                    return Some(n);
                }
            }
            if let Some(n) = v.as_f64() {
                return Some(n);
            }
        }
    }
    None
}
