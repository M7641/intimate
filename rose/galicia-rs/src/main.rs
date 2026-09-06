//! CLI `galicia` — mêmes commandes que le PoC Python.
//!
//!   galicia demo                          # narratif end-to-end
//!   galicia init [--force]                # crée le warehouse + namespaces
//!   galicia seed-customers --n 50
//!   galicia seed-orders 2026-05-15 --n 200
//!   galicia evolve                        # explique l'évolution de schéma
//!   galicia mart                          # (re)construit le mart
//!   galicia tables
//!   galicia snapshots raw.orders
//!   galicia query "SELECT ..."
//!   galicia info

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use galicia::{catalog, pipeline, seed, warehouse};

#[derive(Parser)]
#[command(
    name = "galicia",
    about = "DuckDB + Apache Iceberg sur 'S3' local — alternative low-cost à Redshift"
)]
struct Cli {
    /// Répertoire local jouant le rôle du bucket S3.
    #[arg(long, global = true, default_value = "warehouse")]
    warehouse: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Lance la démo narrative complète (reset le warehouse).
    Demo,
    /// Crée le warehouse et les namespaces raw / mart.
    Init {
        /// Wipe un warehouse existant.
        #[arg(long)]
        force: bool,
    },
    /// Ajoute un lot de customers synthétiques.
    SeedCustomers {
        #[arg(long, default_value_t = 50)]
        n: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Ajoute un lot d'orders pour un jour (YYYY-MM-DD).
    SeedOrders {
        day: String,
        #[arg(long, default_value_t = 200)]
        n: usize,
        #[arg(long, default_value_t = 11)]
        seed: u64,
        #[arg(long)]
        with_discount: bool,
    },
    /// Explique l'évolution de schéma (discount_cents).
    Evolve,
    /// (Re)construit mart.revenue_by_country depuis raw.*.
    Mart,
    /// Liste les tables du warehouse.
    Tables,
    /// Liste les snapshots d'une table (ex: raw.orders).
    Snapshots { table: String },
    /// Exécute un SQL ad-hoc via DuckDB (les tables sont des views).
    Query { sql: String },
    /// Affiche l'empreinte disque du 'bucket'.
    Info,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let wh = cli.warehouse.as_path();

    match cli.cmd {
        Cmd::Demo => galicia::demo::run(wh).await?,

        Cmd::Init { force } => {
            let (store, _) = if force {
                pipeline::reset(wh).await?
            } else {
                pipeline::ensure(wh).await?
            };
            println!("warehouse prêt : {}", store.root().display());
        }

        Cmd::SeedCustomers { n, seed: s } => {
            let (_, cat) = pipeline::ensure(wh).await?;
            let snap = pipeline::seed_customers(&cat, n, s).await?;
            println!("snapshot {snap}  (+{n} customers)");
        }

        Cmd::SeedOrders {
            day,
            n,
            seed: s,
            with_discount,
        } => {
            let (_, cat) = pipeline::ensure(wh).await?;
            let snap =
                pipeline::seed_orders(&cat, seed::day_micros(&day)?, n, s, with_discount).await?;
            println!("snapshot {snap}  (+{n} orders le {day})");
        }

        Cmd::Evolve => {
            pipeline::ensure(wh).await?;
            println!("{}", catalog::EVOLVE_NOTE);
        }

        Cmd::Mart => {
            let (_, cat) = pipeline::ensure(wh).await?;
            let snap = pipeline::build_mart(&cat).await?;
            println!("mart.revenue_by_country snapshot {snap}");
        }

        Cmd::Tables => {
            let (_, cat) = pipeline::ensure(wh).await?;
            for t in pipeline::list_tables(&cat).await? {
                println!("{t}");
            }
        }

        Cmd::Snapshots { table } => {
            let (_, cat) = pipeline::ensure(wh).await?;
            println!("{:<22}{:<18}operation", "snapshot_id", "timestamp_ms");
            for (id, ts, op) in pipeline::snapshots(&cat, &table).await? {
                println!("{id:<22}{ts:<18}{op}");
            }
        }

        Cmd::Query { sql } => {
            let (_, cat) = pipeline::ensure(wh).await?;
            let con = warehouse::open()?;
            let customers = cat.load_table(&catalog::ident("raw.customers")?).await?;
            let orders = cat.load_table(&catalog::ident("raw.orders")?).await?;
            warehouse::register_views(
                &con,
                &[("raw", "customers", &customers), ("raw", "orders", &orders)],
            )
            .await?;
            if let Ok(mart) = cat
                .load_table(&catalog::ident("mart.revenue_by_country")?)
                .await
            {
                warehouse::register_views(&con, &[("mart", "revenue_by_country", &mart)]).await?;
            }
            warehouse::show(&con, &sql)?;
        }

        Cmd::Info => {
            let store = galicia::objstore::ObjectStore::new(wh)?;
            if !store.exists() {
                println!("(pas de warehouse à {})", store.root().display());
                std::process::exit(1);
            }
            let (n, total) = store.footprint()?;
            println!(
                "{}: {n} fichiers, {:.1} KiB",
                store.root().display(),
                total as f64 / 1024.0
            );
        }
    }
    Ok(())
}
