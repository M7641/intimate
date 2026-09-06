//! Démo narrative end-to-end : ingestion → time-travel → « évolution » de
//! schéma → mart. Couche de présentation au-dessus de [`crate::pipeline`].

use std::path::Path;

use anyhow::Result;
use duckdb::Connection;

use crate::{catalog, pipeline, seed, warehouse};

fn section(title: &str) {
    println!("\n\x1b[1m── {title} ──\x1b[0m");
}

fn q(con: &Connection, sql: &str) -> Result<()> {
    println!("\x1b[2msql>\x1b[0m {}", sql.trim());
    warehouse::show(con, sql)
}

/// Recharge les tables `raw.*` et (ré)enregistre les vues DuckDB courantes.
async fn refresh_views(con: &Connection, cat: &std::sync::Arc<dyn iceberg::Catalog>) -> Result<()> {
    let customers = cat.load_table(&catalog::ident("raw.customers")?).await?;
    let orders = cat.load_table(&catalog::ident("raw.orders")?).await?;
    warehouse::register_views(
        con,
        &[("raw", "customers", &customers), ("raw", "orders", &orders)],
    )
    .await
}

pub async fn run(warehouse_path: &Path) -> Result<()> {
    let (store, cat) = pipeline::reset(warehouse_path).await?;

    section("1. Setup — un répertoire jouera le rôle de s3://galicia/");
    println!("bucket    : {}", store.root().display());
    println!("warehouse : {}", store.warehouse_uri());
    println!(
        "catalogue : {} (en prod : Glue / Nessie / REST)",
        store.catalog_uri()
    );

    section("2. Ingestion — customers + orders du jour 1 (writer Iceberg)");
    pipeline::seed_customers(&cat, 50, 1).await?;
    let snap1 =
        pipeline::seed_orders(&cat, seed::day_micros("2026-05-15")?, 200, 11, false).await?;
    println!("snapshot 1 : {snap1}  (200 orders écrits)");

    section("3. Lecture analytique — DuckDB lit les Parquet résolus par le catalogue");
    let con = warehouse::open()?;
    refresh_views(&con, &cat).await?;
    q(
        &con,
        "SELECT status, COUNT(*) AS n, SUM(amount_cents)/100.0 AS revenue_eur
         FROM raw.orders GROUP BY status ORDER BY n DESC",
    )?;

    section("4. Append du jour 2 — nouveau snapshot, l'ancien reste interrogeable");
    let snap2 =
        pipeline::seed_orders(&cat, seed::day_micros("2026-05-16")?, 300, 22, false).await?;
    refresh_views(&con, &cat).await?;
    println!("snapshot 2 : {snap2}  (+300 orders)");
    println!("\ncount au snapshot courant :");
    q(&con, "SELECT COUNT(*) AS n FROM raw.orders")?;
    let n_d1 = pipeline::count_at(&cat, "raw.orders", snap1).await?;
    println!("\ncount au snapshot 1 (time-travel Iceberg) : {n_d1}");

    section("5. « Évolution » de schéma — colonne optionnelle, sans réécriture");
    println!("\x1b[2m{}\x1b[0m", catalog::EVOLVE_NOTE);
    pipeline::seed_orders(&cat, seed::day_micros("2026-05-17")?, 150, 33, true).await?;
    refresh_views(&con, &cat).await?;
    println!("\n+150 orders avec discount populé partiellement");
    q(
        &con,
        "SELECT discount_cents IS NOT NULL AS has_discount, COUNT(*) AS n
         FROM raw.orders GROUP BY 1 ORDER BY 1",
    )?;

    section("6. Transformation dbt-style — DuckDB lit Iceberg, écrit Iceberg");
    println!(
        "\x1b[2m-- même SQL qu'en Redshift ; identique au modèle dbt revenue_by_country.sql\x1b[0m"
    );
    pipeline::build_mart(&cat).await?;
    refresh_views(&con, &cat).await?;
    let mart = cat
        .load_table(&catalog::ident("mart.revenue_by_country")?)
        .await?;
    warehouse::register_views(&con, &[("mart", "revenue_by_country", &mart)]).await?;

    section("7. Lecture du mart");
    q(&con, "SELECT * FROM mart.revenue_by_country")?;

    section("8. Empreinte disque du 'bucket'");
    let (n_files, total) = store.footprint()?;
    println!(
        "{n_files} fichiers, {:.1} KiB au total dans {}",
        total as f64 / 1024.0,
        store.root().display()
    );
    println!("(en prod ces mêmes fichiers vivraient sur S3 à ~0,023 $/Go/mois)");

    Ok(())
}
