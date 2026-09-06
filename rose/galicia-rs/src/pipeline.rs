//! Opérations haut-niveau sur le warehouse. Fonctions « pures » côté logique :
//! pas d'affichage décoratif (la démo et le CLI s'en chargent).

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow_array::cast::AsArray;
use arrow_array::types::Int64Type;
use arrow_schema::SchemaRef;
use iceberg::table::Table;
use iceberg::{Catalog, NamespaceIdent};

use crate::catalog::{self, NAMESPACES};
use crate::objstore::ObjectStore;
use crate::{seed, warehouse, writer};

/// Le SQL du mart — identique à ce qu'on écrirait en Redshift, et identique au
/// modèle dbt `revenue_by_country.sql` (cf. dbt/). Les vues `raw.*` sont
/// enregistrées par [`warehouse::register_views`].
const MART_SQL: &str = "
    SELECT
      c.country,
      COUNT(*) FILTER (WHERE o.status = 'paid')                      AS paid_orders,
      SUM(o.amount_cents) FILTER (WHERE o.status = 'paid') / 100.0   AS gross_revenue_eur,
      COALESCE(SUM(o.discount_cents), 0) / 100.0                     AS total_discount_eur
    FROM raw.orders o
    JOIN raw.customers c USING (customer_id)
    GROUP BY c.country
    ORDER BY gross_revenue_eur DESC
";

/// Ouvre le bucket + le catalogue et garantit les namespaces.
pub async fn ensure(warehouse: &Path) -> Result<(ObjectStore, Arc<dyn Catalog>)> {
    let store = ObjectStore::new(warehouse)?;
    let cat = catalog::open(&store).await?;
    catalog::ensure_namespaces(&cat).await?;
    Ok((store, cat))
}

/// Wipe complet du bucket puis recréation (équivaut à vider le préfixe S3).
pub async fn reset(warehouse: &Path) -> Result<(ObjectStore, Arc<dyn Catalog>)> {
    if warehouse.exists() {
        std::fs::remove_dir_all(warehouse).context("wipe warehouse")?;
    }
    ensure(warehouse).await
}

fn arrow_schema(table: &Table) -> Result<SchemaRef> {
    let schema = iceberg::arrow::schema_to_arrow_schema(table.metadata().current_schema())
        .context("schema_to_arrow_schema")?;
    Ok(Arc::new(schema))
}

fn current_snapshot(table: &Table) -> i64 {
    table
        .metadata()
        .current_snapshot()
        .map(|s| s.snapshot_id())
        .unwrap_or(-1)
}

/// Ajoute un lot de `n` customers à `raw.customers`. Renvoie le snapshot créé.
pub async fn seed_customers(catalog: &Arc<dyn Catalog>, n: usize, seed_value: u64) -> Result<i64> {
    let table =
        catalog::create_if_absent(catalog, "raw.customers", catalog::customers_schema()).await?;
    let aschema = arrow_schema(&table)?;
    let batch = seed::customers_batch(&aschema, n, seed_value);
    let table = writer::append(catalog, &table, batch).await?;
    Ok(current_snapshot(&table))
}

/// Lit tous les `customer_id` du snapshot courant (via le scan Iceberg natif).
pub async fn read_customer_ids(catalog: &Arc<dyn Catalog>) -> Result<Vec<i64>> {
    use futures::TryStreamExt;
    let table = catalog
        .load_table(&catalog::ident("raw.customers")?)
        .await?;
    let scan = table.scan().select(["customer_id"]).build()?;
    let batches = scan.to_arrow().await?.try_collect::<Vec<_>>().await?;
    let mut ids = Vec::new();
    for b in batches {
        let col = b.column(0).as_primitive::<Int64Type>();
        for i in 0..col.len() {
            ids.push(col.value(i));
        }
    }
    Ok(ids)
}

/// Ajoute un lot de `n` orders pour `day_micros`. Renvoie le snapshot créé.
pub async fn seed_orders(
    catalog: &Arc<dyn Catalog>,
    day_micros: i64,
    n: usize,
    seed_value: u64,
    with_discount: bool,
) -> Result<i64> {
    let cids = read_customer_ids(catalog).await?;
    if cids.is_empty() {
        anyhow::bail!("raw.customers est vide — seed-customers d'abord.");
    }
    let table = catalog::create_if_absent(catalog, "raw.orders", catalog::orders_schema()).await?;
    let aschema = arrow_schema(&table)?;
    let batch = seed::orders_batch(&aschema, n, &cids, day_micros, seed_value, with_discount);
    let table = writer::append(catalog, &table, batch).await?;
    Ok(current_snapshot(&table))
}

/// (Re)construit `mart.revenue_by_country` : DuckDB lit `raw.*`, agrège, et le
/// résultat est réécrit comme table Iceberg. apache/iceberg-rust 0.9 n'a pas
/// d'action `overwrite` ; on émule donc drop + recreate + append.
pub async fn build_mart(catalog: &Arc<dyn Catalog>) -> Result<i64> {
    let orders = catalog.load_table(&catalog::ident("raw.orders")?).await?;
    let customers = catalog
        .load_table(&catalog::ident("raw.customers")?)
        .await?;

    let con = warehouse::open()?;
    warehouse::register_views(
        &con,
        &[("raw", "customers", &customers), ("raw", "orders", &orders)],
    )
    .await?;

    let rows: Vec<(String, i64, f64, f64)> = {
        let mut stmt = con.prepare(MART_SQL)?;
        let mapped = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, f64>(3)?,
            ))
        })?;
        mapped.collect::<std::result::Result<Vec<_>, _>>()?
    };

    let mart_id = catalog::ident("mart.revenue_by_country")?;
    if catalog.table_exists(&mart_id).await? {
        catalog.drop_table(&mart_id).await?;
    }
    let table =
        catalog::create_if_absent(catalog, "mart.revenue_by_country", catalog::mart_schema())
            .await?;
    let aschema = arrow_schema(&table)?;
    let batch = seed::mart_batch(&aschema, &rows);
    let table = writer::append(catalog, &table, batch).await?;
    Ok(current_snapshot(&table))
}

/// Liste les tables logiques de tous les namespaces.
pub async fn list_tables(catalog: &Arc<dyn Catalog>) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for ns in NAMESPACES {
        let nsident = NamespaceIdent::new(ns.to_string());
        if !catalog.namespace_exists(&nsident).await? {
            continue;
        }
        for t in catalog.list_tables(&nsident).await? {
            out.push(format!("{ns}.{}", t.name()));
        }
    }
    Ok(out)
}

/// Liste les snapshots d'une table : (snapshot_id, timestamp_ms, opération).
pub async fn snapshots(
    catalog: &Arc<dyn Catalog>,
    dotted: &str,
) -> Result<Vec<(i64, i64, String)>> {
    let table = catalog.load_table(&catalog::ident(dotted)?).await?;
    let mut out: Vec<(i64, i64, String)> = table
        .metadata()
        .snapshots()
        .map(|s| {
            (
                s.snapshot_id(),
                s.timestamp_ms(),
                format!("{:?}", s.summary().operation),
            )
        })
        .collect();
    out.sort_by_key(|t| t.1);
    Ok(out)
}

/// Compte le nombre de lignes d'une table à un snapshot donné (time-travel),
/// via le scan Iceberg natif (pas besoin de DuckDB pour un simple count).
pub async fn count_at(catalog: &Arc<dyn Catalog>, dotted: &str, snapshot: i64) -> Result<usize> {
    use futures::TryStreamExt;
    let table = catalog.load_table(&catalog::ident(dotted)?).await?;
    let scan = table.scan().snapshot_id(snapshot).select_all().build()?;
    let batches = scan.to_arrow().await?.try_collect::<Vec<_>>().await?;
    Ok(batches.iter().map(|b| b.num_rows()).sum())
}
