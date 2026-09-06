//! The normalized query-plan model.
//!
//! Postgres and DuckDB describe a plan in very different vocabularies:
//! Postgres speaks in *estimated cost* (an abstract unit calibrated on
//! `seq_page_cost`), DuckDB speaks in *cardinality* and per-operator *timing*.
//! Everything downstream — the API, the React tree, the cost highlights —
//! works against this one shape, so each engine's job is simply to fold its
//! native JSON into a [`PlanNode`] tree.

use serde::Serialize;

/// One operator in the plan tree (a scan, a join, a sort, ...).
#[derive(Debug, Clone, Serialize)]
pub struct PlanNode {
    /// Stable pre-order id, used by [`Note`] to point at a node.
    pub id: usize,
    /// Human operator name, e.g. `Seq Scan`, `HASH_JOIN`.
    pub operator: String,
    /// A short qualifier: relation name, join condition, index used.
    pub detail: Option<String>,
    /// Rows the planner *expects* this operator to emit.
    pub est_rows: Option<f64>,
    /// Estimated total cost. Postgres only — DuckDB has no cost currency.
    pub est_cost: Option<f64>,
    /// Rows actually emitted (only when the query was ANALYZE-d).
    pub actual_rows: Option<f64>,
    /// Wall-clock time spent in this operator, milliseconds (ANALYZE only).
    pub actual_ms: Option<f64>,
    /// The single most expensive operator in the tree is flagged here so the
    /// UI can draw the eye straight to the bottleneck.
    pub hot: bool,
    pub children: Vec<PlanNode>,
}

impl PlanNode {
    pub fn new(operator: impl Into<String>) -> Self {
        Self {
            id: 0,
            operator: operator.into(),
            detail: None,
            est_rows: None,
            est_cost: None,
            actual_rows: None,
            actual_ms: None,
            hot: false,
            children: Vec::new(),
        }
    }

    /// Walk the tree in pre-order, handing each node its `id`. Returns the
    /// number of nodes seen so callers can report a node count.
    pub fn assign_ids(&mut self) -> usize {
        fn walk(node: &mut PlanNode, next: &mut usize) {
            node.id = *next;
            *next += 1;
            for child in &mut node.children {
                walk(child, next);
            }
        }
        let mut next = 0;
        walk(self, &mut next);
        next
    }

    fn visit<'a>(&'a self, out: &mut Vec<&'a PlanNode>) {
        out.push(self);
        for child in &self.children {
            child.visit(out);
        }
    }

    fn visit_mut(&mut self, f: &mut impl FnMut(&mut PlanNode)) {
        f(self);
        for child in &mut self.children {
            child.visit_mut(f);
        }
    }
}

/// A one-line observation about the plan — an educational hint ("this is a
/// sequential scan, an index might help") or a neutral fact ("columnar scans
/// are the normal path in DuckDB").
#[derive(Debug, Clone, Serialize)]
pub struct Note {
    pub level: NoteLevel,
    /// The node this note is about, or `None` for a plan-wide remark.
    pub node_id: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteLevel {
    Info,
    Warn,
}

/// Roll-up numbers shown at the top of each engine panel.
#[derive(Debug, Clone, Serialize)]
pub struct PlanSummary {
    pub total_cost: Option<f64>,
    pub est_rows: Option<f64>,
    pub node_count: usize,
    pub total_ms: Option<f64>,
}

/// The full result of explaining one query on one engine.
#[derive(Debug, Clone, Serialize)]
pub struct PlanResult {
    pub root: PlanNode,
    /// The engine's own EXPLAIN output, pretty-printed, for the "raw" tab.
    pub raw: String,
    pub summary: PlanSummary,
    pub notes: Vec<Note>,
}

impl PlanResult {
    /// Finish a freshly parsed tree: assign ids, find the bottleneck, and
    /// build the summary. Engine-specific hints are layered on top by the
    /// caller via [`PlanResult::with_note`].
    pub fn finalize(mut root: PlanNode, raw: String) -> Self {
        let node_count = root.assign_ids();

        // Self-cost = a node's total cost minus what its children already
        // account for. That is what actually makes one operator "the
        // expensive one", rather than the cumulative root cost.
        let hot_id = pick_hot(&root);
        if let Some(id) = hot_id {
            root.visit_mut(&mut |n| {
                if n.id == id {
                    n.hot = true;
                }
            });
        }

        let mut flat = Vec::new();
        root.visit(&mut flat);
        let total_ms = flat.iter().filter_map(|n| n.actual_ms).fold(None, sum_opt);

        let summary = PlanSummary {
            total_cost: root.est_cost,
            est_rows: root.est_rows,
            node_count,
            total_ms,
        };

        PlanResult { root, raw, summary, notes: Vec::new() }
    }

    pub fn with_note(mut self, level: NoteLevel, node_id: Option<usize>, message: impl Into<String>) -> Self {
        self.notes.push(Note { level, node_id, message: message.into() });
        self
    }
}

fn sum_opt(acc: Option<f64>, v: f64) -> Option<f64> {
    Some(acc.unwrap_or(0.0) + v)
}

/// Choose the bottleneck node. We prefer real measurements when we have them
/// (ANALYZE timing), then fall back to estimated self-cost, then to rows.
fn pick_hot(root: &PlanNode) -> Option<usize> {
    let mut flat = Vec::new();
    root.visit(&mut flat);

    // 1. Actual time, if the plan was analyzed.
    if let Some(n) = flat
        .iter()
        .filter(|n| n.actual_ms.is_some())
        .max_by(|a, b| a.actual_ms.partial_cmp(&b.actual_ms).unwrap())
    {
        return Some(n.id);
    }

    // 2. Estimated self-cost (Postgres). Total cost is cumulative, so subtract
    //    the children's total to isolate what each node adds on its own.
    let self_cost = |n: &PlanNode| -> Option<f64> {
        let total = n.est_cost?;
        let children: f64 = n.children.iter().filter_map(|c| c.est_cost).sum();
        Some((total - children).max(0.0))
    };
    if let Some(n) = flat
        .iter()
        .filter(|n| n.est_cost.is_some())
        .max_by(|a, b| self_cost(a).partial_cmp(&self_cost(b)).unwrap())
    {
        return Some(n.id);
    }

    // 3. Estimated rows (DuckDB, plan-only).
    flat.iter()
        .filter(|n| n.est_rows.is_some())
        .max_by(|a, b| a.est_rows.partial_cmp(&b.est_rows).unwrap())
        .map(|n| n.id)
}
