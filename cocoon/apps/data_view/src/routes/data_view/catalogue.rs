//! Catalogue map — the "whole database" view.
//!
//! A warehouse does not *enforce* foreign keys, so relationships between tables
//! come from two sources, strongest first:
//!
//! 1. **Declared** constraints. Redshift and Snowflake both let you *define*
//!    `FOREIGN KEY`s (informational / NOT ENFORCED). When present, these are
//!    ground truth: someone stated the relationship on purpose, with direction
//!    and exact column pairing.
//! 2. **Inferred** links. Where no constraint is declared, two tables sharing a
//!    key-like column name (`customer_id`, `sku`, …) are linked as a guess.
//!
//! Routes:
//! - `GET /catalogue` (light) — one pass over `information_schema` builds the
//!   graph: tables as nodes, declared + inferred links as edges.
//! - `GET /link_overlap` (heavy) — on demand, validate one link by counting how
//!   many distinct values its column pair actually shares across the two tables'
//!   latest snapshots. Turns a guess into a measured confidence.

use std::collections::{BTreeMap, HashSet};

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::validation::{
    EXCLUDED_COLUMNS, build_timestamp_where_clause, build_valid_columns_query,
    table_has_load_timestamp, validate_column_name, validate_schema_name, validate_table_name,
};

// ── Tuning ────────────────────────────────────────────────────────────

/// A shared column name present in more tables than this is treated as a
/// generic surrogate key (omnipresent, e.g. a warehouse-wide `record_id`)
/// rather than a meaningful relationship. Linking every such pair would
/// produce an unreadable clique, so those columns are dropped from inferred
/// linking and reported in `omitted_columns` so the omission is never silent.
const MAX_SHARED_TABLES: usize = 12;

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, IntoParams)]
pub struct SchemaQueryParam {
    #[serde(default = "default_schema")]
    pub schema: String,
}

fn default_schema() -> String {
    "stage".to_string()
}

/// One table in the catalogue graph.
#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogueNode {
    pub table_name: String,
    pub column_count: usize,
    /// The key-like columns on this table (these drive inferred linking).
    pub key_columns: Vec<String>,
}

/// How an edge was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// A `FOREIGN KEY` declared in the catalogue (directed, high confidence).
    Declared,
    /// A shared key-like column name (undirected guess).
    Inferred,
}

/// One column pairing carried by an edge. For inferred links the two names are
/// identical; declared foreign keys may pair differently named columns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct EdgeColumn {
    pub source_column: String,
    pub target_column: String,
}

/// An link between two tables. For `Declared` edges the direction is
/// child (`source`) → parent (`target`); inferred edges are undirected.
#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogueEdge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    pub links: Vec<EdgeColumn>,
}

/// A key-like column omitted from inferred linking because it spans too many
/// tables.
#[derive(Debug, Serialize, ToSchema)]
pub struct OmittedColumn {
    pub column: String,
    pub table_count: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogueResponse {
    pub schema: String,
    pub nodes: Vec<CatalogueNode>,
    pub edges: Vec<CatalogueEdge>,
    /// Key-like columns omitted from inferred linking (too generic).
    pub omitted_columns: Vec<OmittedColumn>,
}

/// A foreign key declared in the catalogue (child → parent).
#[derive(Debug, Clone)]
struct DeclaredFk {
    source_table: String,
    source_column: String,
    target_table: String,
    target_column: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct LinkOverlapParams {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub source_table: String,
    pub target_table: String,
    /// Column on the source side.
    pub source_column: String,
    /// Column on the target side. Defaults to `source_column` when omitted
    /// (the inferred-link case, where both names match).
    pub target_column: Option<String>,
}

/// The measured value overlap that validates a link.
#[derive(Debug, Serialize, ToSchema)]
pub struct LinkOverlapResponse {
    pub source_table: String,
    pub target_table: String,
    pub source_column: String,
    pub target_column: String,
    pub source_distinct: i64,
    pub target_distinct: i64,
    pub overlap: i64,
    /// Share of the source's distinct values also found in the target (0–100).
    pub source_coverage_pct: f64,
    /// Share of the target's distinct values also found in the source (0–100).
    pub target_coverage_pct: f64,
}

// ── Key-like column heuristic ─────────────────────────────────────────

/// Is this column name likely to carry a join key?
///
/// We accept the common warehouse conventions — anything ending in `_id`,
/// `_key` or `_code`, plus a few bare names (`sku`, `uuid`, `guid`). A bare
/// `id` is deliberately excluded: it appears on almost every table as a local
/// surrogate key and would link everything to everything.
fn is_key_like(name: &str) -> bool {
    let lower = name.to_lowercase();
    if EXCLUDED_COLUMNS.contains(lower.as_str()) {
        return false;
    }
    lower.ends_with("_id")
        || lower.ends_with("_key")
        || lower.ends_with("_code")
        || matches!(lower.as_str(), "sku" | "uuid" | "guid")
}

/// Unordered key for a table pair, so (a,b) and (b,a) collapse to one edge.
fn pair_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

// ── Query builders ────────────────────────────────────────────────────

/// All columns of every table in a schema, ordered for stable output.
fn build_schema_columns_query(schema: &str) -> String {
    let schema_lower = schema.to_lowercase();
    format!(
        "SELECT table_name, column_name \
         FROM information_schema.columns \
         WHERE lower(table_schema) = '{schema_lower}' \
         ORDER BY table_name, ordinal_position"
    )
}

/// Declared foreign keys in a schema, child columns → parent columns.
///
/// Standard `information_schema` joins, supported (informationally) by both
/// Redshift and Snowflake. If a backend lacks these views the query errors and
/// the caller falls back to inferred links only.
fn build_declared_fk_query(schema: &str) -> String {
    let schema_lower = schema.to_lowercase();
    format!(
        "SELECT \
           kcu.table_name  AS source_table, \
           kcu.column_name AS source_column, \
           ccu.table_name  AS target_table, \
           ccu.column_name AS target_column \
         FROM information_schema.table_constraints tc \
         JOIN information_schema.key_column_usage kcu \
           ON tc.constraint_name = kcu.constraint_name \
          AND tc.table_schema = kcu.table_schema \
         JOIN information_schema.constraint_column_usage ccu \
           ON tc.constraint_name = ccu.constraint_name \
          AND tc.table_schema = ccu.table_schema \
         WHERE tc.constraint_type = 'FOREIGN KEY' \
           AND lower(tc.table_schema) = '{schema_lower}'"
    )
}

/// One query returning the three counts that define an edge's overlap.
///
/// Each side is cast to VARCHAR (so an `int` ↔ `varchar` pairing still
/// compares) and pinned to its own latest snapshot via `where_*`.
fn build_overlap_query(
    schema: &str,
    source: &str,
    source_column: &str,
    target: &str,
    target_column: &str,
    where_source: &str,
    where_target: &str,
) -> String {
    let src = format!("CAST(\"{source_column}\" AS VARCHAR)");
    let tgt = format!("CAST(\"{target_column}\" AS VARCHAR)");
    format!(
        "SELECT \
           (SELECT COUNT(DISTINCT {src}) FROM {schema}.\"{source}\" {where_source}) AS source_distinct, \
           (SELECT COUNT(DISTINCT {tgt}) FROM {schema}.\"{target}\" {where_target}) AS target_distinct, \
           (SELECT COUNT(*) FROM ( \
              SELECT DISTINCT {src} AS v FROM {schema}.\"{source}\" {where_source} \
              INTERSECT \
              SELECT DISTINCT {tgt} AS v FROM {schema}.\"{target}\" {where_target} \
           ) ov) AS overlap"
    )
}

// ── Graph construction ────────────────────────────────────────────────

/// Build inferred edges from shared key-like column names.
///
/// Returns the edges plus the columns omitted for being too widespread. Pairs
/// already covered by a declared FK are skipped — the declared edge supersedes.
fn build_inferred_edges(
    table_columns: &BTreeMap<String, Vec<String>>,
    declared_pairs: &HashSet<(String, String)>,
) -> (Vec<CatalogueEdge>, Vec<OmittedColumn>) {
    // Which tables carry each key-like column name?
    let mut column_tables: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (table, columns) in table_columns {
        for col in columns {
            if is_key_like(col) {
                column_tables
                    .entry(col.to_lowercase())
                    .or_default()
                    .push(table.clone());
            }
        }
    }

    let mut pair_columns: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut omitted_columns = Vec::new();
    for (column, mut tables) in column_tables {
        tables.sort();
        tables.dedup();
        if tables.len() < 2 {
            continue;
        }
        if tables.len() > MAX_SHARED_TABLES {
            omitted_columns.push(OmittedColumn {
                column,
                table_count: tables.len(),
            });
            continue;
        }
        for i in 0..tables.len() {
            for j in (i + 1)..tables.len() {
                let key = pair_key(&tables[i], &tables[j]);
                if declared_pairs.contains(&key) {
                    continue; // a declared FK already links these two
                }
                pair_columns.entry(key).or_default().push(column.clone());
            }
        }
    }

    let edges = pair_columns
        .into_iter()
        .map(|((source, target), columns)| CatalogueEdge {
            source,
            target,
            kind: EdgeKind::Inferred,
            links: columns
                .into_iter()
                .map(|c| EdgeColumn {
                    source_column: c.clone(),
                    target_column: c,
                })
                .collect(),
        })
        .collect();

    (edges, omitted_columns)
}

/// Fold declared FKs into edges, merging multiple columns of the same FK pair.
fn build_declared_edges(fks: &[DeclaredFk]) -> Vec<CatalogueEdge> {
    let mut by_pair: BTreeMap<(String, String), Vec<EdgeColumn>> = BTreeMap::new();
    for fk in fks {
        by_pair
            .entry((fk.source_table.clone(), fk.target_table.clone()))
            .or_default()
            .push(EdgeColumn {
                source_column: fk.source_column.clone(),
                target_column: fk.target_column.clone(),
            });
    }
    by_pair
        .into_iter()
        .map(|((source, target), links)| CatalogueEdge {
            source,
            target,
            kind: EdgeKind::Declared,
            links,
        })
        .collect()
}

/// Assemble the full graph from the column listing and any declared FKs.
fn build_graph(
    schema: &str,
    table_columns: BTreeMap<String, Vec<String>>,
    declared_fks: Vec<DeclaredFk>,
) -> CatalogueResponse {
    let declared_pairs: HashSet<(String, String)> = declared_fks
        .iter()
        .map(|fk| pair_key(&fk.source_table, &fk.target_table))
        .collect();

    let (inferred_edges, omitted_columns) = build_inferred_edges(&table_columns, &declared_pairs);
    let mut edges = build_declared_edges(&declared_fks);
    edges.extend(inferred_edges);

    let nodes = table_columns
        .into_iter()
        .map(|(table_name, columns)| {
            let key_columns: Vec<String> =
                columns.iter().filter(|c| is_key_like(c)).cloned().collect();
            CatalogueNode {
                table_name,
                column_count: columns.len(),
                key_columns,
            }
        })
        .collect();

    CatalogueResponse {
        schema: schema.to_string(),
        nodes,
        edges,
        omitted_columns,
    }
}

// ── Handlers ──────────────────────────────────────────────────────────

/// Catalogue map: tables as nodes, declared + inferred links as edges.
#[utoipa::path(
    get,
    path = "/api/data_view/catalogue",
    tag = "Data View - Database",
    params(SchemaQueryParam),
    responses(
        (status = 200, description = "Catalogue graph", body = CatalogueResponse),
        (status = 400, description = "Invalid schema name", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state))]
pub async fn catalogue_handler(
    State(state): State<AppState>,
    Query(params): Query<SchemaQueryParam>,
) -> Result<Json<CatalogueResponse>, AppError> {
    validate_schema_name(&params.schema)?;

    let sql = build_schema_columns_query(&params.schema);
    let rows = state.blocking_query(sql).await?;

    // Group columns by table, preserving ordinal order from the query.
    let mut table_columns: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in rows {
        let table = row.get("table_name").and_then(|v| v.as_str());
        let column = row.get("column_name").and_then(|v| v.as_str());
        if let (Some(table), Some(column)) = (table, column) {
            table_columns
                .entry(table.to_string())
                .or_default()
                .push(column.to_string());
        }
    }

    // Declared FKs are a bonus: a backend without these catalogue views simply
    // yields none, and the graph falls back to inferred links.
    let declared_fks = fetch_declared_fks(&state, &params.schema)
        .await
        .unwrap_or_default();

    Ok(Json(build_graph(
        &params.schema,
        table_columns,
        declared_fks,
    )))
}

/// Validate one link by measuring real value overlap on the column pair.
#[utoipa::path(
    get,
    path = "/api/data_view/link_overlap",
    tag = "Data View - Database",
    params(LinkOverlapParams),
    responses(
        (status = 200, description = "Value overlap for the column pair", body = LinkOverlapResponse),
        (status = 400, description = "Invalid identifier", body = ErrorResponse),
        (status = 500, description = "Database error", body = ErrorResponse)
    )
)]
#[tracing::instrument(skip(state, params))]
pub async fn link_overlap_handler(
    State(state): State<AppState>,
    Query(params): Query<LinkOverlapParams>,
) -> Result<Json<LinkOverlapResponse>, AppError> {
    validate_schema_name(&params.schema)?;
    validate_table_name(&params.source_table)?;
    validate_table_name(&params.target_table)?;

    let target_column = params
        .target_column
        .clone()
        .unwrap_or_else(|| params.source_column.clone());

    // Each column must be a safe identifier AND exist in its table.
    validate_column_name(
        &params.source_column,
        &fetch_columns(&state, &params.schema, &params.source_table).await?,
    )?;
    validate_column_name(
        &target_column,
        &fetch_columns(&state, &params.schema, &params.target_table).await?,
    )?;

    // Pin each side to its own latest snapshot when it is event-sourced.
    let source_has_ts =
        table_has_load_timestamp(&state, &params.schema, &params.source_table).await?;
    let target_has_ts =
        table_has_load_timestamp(&state, &params.schema, &params.target_table).await?;
    let where_source =
        build_timestamp_where_clause(&params.schema, &params.source_table, None, source_has_ts);
    let where_target =
        build_timestamp_where_clause(&params.schema, &params.target_table, None, target_has_ts);

    let sql = build_overlap_query(
        &params.schema,
        &params.source_table,
        &params.source_column,
        &params.target_table,
        &target_column,
        &where_source,
        &where_target,
    );
    let rows = state.blocking_query(sql).await?;

    let row = rows
        .first()
        .ok_or_else(|| AppError::Internal("overlap query returned no rows".into()))?;
    let source_distinct = row
        .get("source_distinct")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let target_distinct = row
        .get("target_distinct")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let overlap = row.get("overlap").and_then(|v| v.as_i64()).unwrap_or(0);

    let coverage = |total: i64| {
        if total > 0 {
            (overlap as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    };

    Ok(Json(LinkOverlapResponse {
        source_table: params.source_table,
        target_table: params.target_table,
        source_column: params.source_column,
        target_column,
        source_distinct,
        target_distinct,
        overlap,
        source_coverage_pct: coverage(source_distinct),
        target_coverage_pct: coverage(target_distinct),
    }))
}

/// Fetch declared foreign keys, keeping only safe-identifier rows.
async fn fetch_declared_fks(state: &AppState, schema: &str) -> Result<Vec<DeclaredFk>, AppError> {
    let sql = build_declared_fk_query(schema);
    let rows = state.blocking_query(sql).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let get = |key: &str| row.get(key).and_then(|v| v.as_str()).map(String::from);
            Some(DeclaredFk {
                source_table: get("source_table")?,
                source_column: get("source_column")?,
                target_table: get("target_table")?,
                target_column: get("target_column")?,
            })
        })
        // Only safe identifiers are allowed into the graph (and later overlap).
        .filter(|fk| {
            validate_table_name(&fk.source_table).is_ok()
                && validate_table_name(&fk.target_table).is_ok()
        })
        .collect())
}

/// Fetch the real column-name set for a table (for column validation).
async fn fetch_columns(
    state: &AppState,
    schema: &str,
    table: &str,
) -> Result<HashSet<String>, AppError> {
    let sql = build_valid_columns_query(schema, table);
    let rows = state.blocking_query(sql).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            row.get("column_name")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cols(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(t, cs)| {
                (
                    t.to_string(),
                    cs.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
                )
            })
            .collect()
    }

    #[test]
    fn key_like_accepts_conventional_keys() {
        assert!(is_key_like("customer_id"));
        assert!(is_key_like("order_key"));
        assert!(is_key_like("product_code"));
        assert!(is_key_like("sku"));
        assert!(is_key_like("SKU"));
    }

    #[test]
    fn key_like_rejects_generic_and_metadata() {
        assert!(!is_key_like("id")); // bare id is too generic
        assert!(!is_key_like("name"));
        assert!(!is_key_like("amount"));
        assert!(!is_key_like("batch_group_id")); // excluded metadata column
        assert!(!is_key_like("row_hash"));
    }

    #[test]
    fn graph_links_tables_sharing_a_key() {
        let tc = cols(&[
            ("customers", &["customer_id", "name"]),
            ("orders", &["order_id", "customer_id"]),
        ]);
        let g = build_graph("stage", tc, vec![]);

        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);
        let edge = &g.edges[0];
        assert_eq!(edge.kind, EdgeKind::Inferred);
        assert_eq!(edge.source, "customers");
        assert_eq!(edge.target, "orders");
        assert_eq!(edge.links.len(), 1);
        assert_eq!(edge.links[0].source_column, "customer_id");
        assert_eq!(edge.links[0].target_column, "customer_id");
    }

    #[test]
    fn graph_omits_overly_shared_columns() {
        // `tenant_id` on more than MAX_SHARED_TABLES tables is a surrogate key,
        // not a relationship — it must be omitted, not exploded into a clique.
        let mut entries: Vec<(String, Vec<String>)> = Vec::new();
        for i in 0..(MAX_SHARED_TABLES + 1) {
            entries.push((
                format!("t{i}"),
                vec!["tenant_id".to_string(), format!("local_{i}_code")],
            ));
        }
        let tc: BTreeMap<String, Vec<String>> = entries.into_iter().collect();
        let g = build_graph("stage", tc, vec![]);

        assert!(
            g.edges
                .iter()
                .all(|e| e.links.iter().all(|l| l.source_column != "tenant_id"))
        );
        assert_eq!(g.omitted_columns.len(), 1);
        assert_eq!(g.omitted_columns[0].column, "tenant_id");
    }

    #[test]
    fn declared_fk_supersedes_inferred_for_same_pair() {
        let tc = cols(&[
            ("orders", &["order_id", "customer_id"]),
            ("customers", &["customer_id", "name"]),
        ]);
        let fks = vec![DeclaredFk {
            source_table: "orders".to_string(),
            source_column: "customer_id".to_string(),
            target_table: "customers".to_string(),
            target_column: "customer_id".to_string(),
        }];
        let g = build_graph("stage", tc, fks);

        // One edge only, and it is the directed declared one.
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].kind, EdgeKind::Declared);
        assert_eq!(g.edges[0].source, "orders");
        assert_eq!(g.edges[0].target, "customers");
    }

    #[test]
    fn declared_fk_keeps_distinct_column_names() {
        let fks = vec![DeclaredFk {
            source_table: "orders".to_string(),
            source_column: "cust_ref".to_string(),
            target_table: "customers".to_string(),
            target_column: "id".to_string(),
        }];
        let edges = build_declared_edges(&fks);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].links[0].source_column, "cust_ref");
        assert_eq!(edges[0].links[0].target_column, "id");
    }
}
