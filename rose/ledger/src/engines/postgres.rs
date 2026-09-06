//! The transactional engine: Postgres over the wire.
//!
//! Postgres is a row-store with a cost-based planner. Its `EXPLAIN (FORMAT
//! JSON)` gives us estimated `Total Cost` and `Plan Rows` per node; adding
//! `ANALYZE` executes the query and adds `Actual Rows` / `Actual Total Time`.
//! We connect with the pure-Rust `tokio-postgres` (no libpq to install).

use crate::engine::{Engine, EngineKind};
use crate::plan::{NoteLevel, PlanNode, PlanResult};
use anyhow::{anyhow, Context};
use serde_json::Value;
use std::sync::Arc;
use tokio_postgres::{Client, NoTls};

pub struct PostgresEngine {
    client: Arc<Client>,
}

impl PostgresEngine {
    /// Connect and spawn the connection driver task. The returned engine holds
    /// only the `Client`, which is cheap to share across requests.
    pub async fn connect(conn_str: &str) -> anyhow::Result<Self> {
        let (client, connection) = tokio_postgres::connect(conn_str, NoTls)
            .await
            .context("connect to Postgres")?;

        // The connection object drives the socket; it must be polled for the
        // client to work, so it lives on its own task for the process lifetime.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!(error = %e, "postgres connection error");
            }
        });

        Ok(Self { client: Arc::new(client) })
    }
}

#[async_trait::async_trait]
impl Engine for PostgresEngine {
    fn id(&self) -> &'static str {
        "postgres"
    }
    fn name(&self) -> &'static str {
        "Postgres"
    }
    fn kind(&self) -> EngineKind {
        EngineKind::Transactional
    }

    async fn explain(&self, sql: &str, analyze: bool) -> anyhow::Result<PlanResult> {
        // VERBOSE + COSTS give us relation names and cost numbers; BUFFERS is
        // only meaningful once we actually execute under ANALYZE.
        let options = if analyze {
            "ANALYZE, VERBOSE, COSTS, BUFFERS, FORMAT JSON"
        } else {
            "VERBOSE, COSTS, FORMAT JSON"
        };
        let stmt = format!("EXPLAIN ({options}) {sql}");

        // FORMAT JSON packs the whole plan into a single `json`-typed column;
        // `try_get` reads it as a Value (and, unlike `get`, returns an error
        // instead of panicking if the shape ever surprises us).
        let row = self
            .client
            .query_one(&stmt, &[])
            .await
            .context("run EXPLAIN on Postgres")?;
        let value: Value = row.try_get(0).context("read Postgres plan JSON column")?;

        let plan = value
            .as_array()
            .and_then(|a| a.first())
            .and_then(|o| o.get("Plan"))
            .ok_or_else(|| anyhow!("unexpected Postgres EXPLAIN shape"))?;

        let root = parse_node(plan);
        let pretty = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());

        // Collect cost hints in the same pre-order the tree is numbered in, so
        // the ids line up with the finalized nodes.
        let hints = collect_hints(plan);

        let mut result = PlanResult::finalize(root, pretty);
        for (id, level, msg) in hints {
            result = result.with_note(level, Some(id), msg);
        }
        Ok(result)
    }
}

fn parse_node(plan: &Value) -> PlanNode {
    let operator = plan
        .get("Node Type")
        .and_then(Value::as_str)
        .unwrap_or("Node")
        .to_string();
    let mut node = PlanNode::new(operator);

    node.detail = detail(plan);
    node.est_rows = plan.get("Plan Rows").and_then(Value::as_f64);
    node.est_cost = plan.get("Total Cost").and_then(Value::as_f64);
    node.actual_rows = plan.get("Actual Rows").and_then(Value::as_f64);
    node.actual_ms = plan.get("Actual Total Time").and_then(Value::as_f64);

    if let Some(children) = plan.get("Plans").and_then(Value::as_array) {
        node.children = children.iter().map(parse_node).collect();
    }
    node
}

/// A short qualifier for the operator: what relation/index it touches, or the
/// join it performs.
fn detail(plan: &Value) -> Option<String> {
    if let Some(rel) = plan.get("Relation Name").and_then(Value::as_str) {
        let alias = plan.get("Index Name").and_then(Value::as_str);
        return Some(match alias {
            Some(idx) => format!("on {rel} using {idx}"),
            None => format!("on {rel}"),
        });
    }
    if let Some(join) = plan.get("Join Type").and_then(Value::as_str) {
        return Some(format!("{join} join"));
    }
    plan.get("Index Name")
        .and_then(Value::as_str)
        .map(|idx| format!("using {idx}"))
}

/// Walk the raw plan in pre-order, mirroring [`PlanNode::assign_ids`], and emit
/// row-store cost hints. The counter is the node id each hint attaches to.
fn collect_hints(root: &Value) -> Vec<(usize, NoteLevel, String)> {
    let mut hints = Vec::new();
    let mut id = 0usize;
    walk_hints(root, &mut id, &mut hints);
    hints
}

fn walk_hints(plan: &Value, id: &mut usize, out: &mut Vec<(usize, NoteLevel, String)>) {
    let my_id = *id;
    *id += 1;

    let node_type = plan.get("Node Type").and_then(Value::as_str).unwrap_or("");
    let rows = plan.get("Plan Rows").and_then(Value::as_f64).unwrap_or(0.0);

    match node_type {
        "Seq Scan" if rows > 10_000.0 => {
            let rel = plan.get("Relation Name").and_then(Value::as_str).unwrap_or("the table");
            out.push((
                my_id,
                NoteLevel::Warn,
                format!(
                    "Sequential scan on {rel} (~{rows:.0} rows). In a row-store this reads \
                     every page — an index on the filtered/joined column can turn it into an \
                     index scan."
                ),
            ));
        }
        "Nested Loop" if rows > 100_000.0 => {
            out.push((
                my_id,
                NoteLevel::Warn,
                format!(
                    "Nested loop over ~{rows:.0} rows. This re-probes the inner side per outer \
                     row; a hash or merge join is usually cheaper at this size."
                ),
            ));
        }
        "Sort" => {
            let method = plan.get("Sort Method").and_then(Value::as_str);
            if let Some(m) = method {
                if m.contains("external") {
                    out.push((
                        my_id,
                        NoteLevel::Warn,
                        format!("Sort spilled to disk ({m}). Raising work_mem may keep it in memory."),
                    ));
                }
            } else {
                out.push((my_id, NoteLevel::Info, "Explicit sort step.".to_string()));
            }
        }
        _ => {}
    }

    if let Some(children) = plan.get("Plans").and_then(Value::as_array) {
        for child in children {
            walk_hints(child, id, out);
        }
    }
}
