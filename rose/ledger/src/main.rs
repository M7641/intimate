//! ledger — explain one SQL query across an analytical engine (DuckDB) and a
//! transactional one (Postgres) side by side, and highlight where the cost is.
//!
//! Both engines are hidden behind the [`engine::Engine`] trait, so adding
//! Redshift or Snowflake later is one more `impl` and one more line in
//! [`build_registry`] — the API and the frontend do not change.

mod api;
mod engine;
mod engines;
mod plan;

use crate::api::AppState;
use crate::engine::{Engine, Registry};
use crate::engines::duckdb::DuckDbEngine;
use crate::engines::postgres::PostgresEngine;
use anyhow::Context;
use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

/// The schema + data that seeds the in-memory DuckDB. The Postgres container
/// seeds itself from the sibling `seed/postgres.sql`; the two must stay in
/// step so the same query is genuinely comparable across engines.
const DUCKDB_SEED: &str = include_str!("../seed/duckdb.sql");

#[derive(Parser)]
#[command(name = "ledger", about = "Compare SQL query plans across engines")]
struct Cli {
    /// Address the HTTP API listens on.
    #[arg(long, env = "LEDGER_ADDR", default_value = "127.0.0.1:47000")]
    addr: String,

    /// Postgres connection string. Matches the bundled docker-compose default.
    #[arg(
        long,
        env = "DATABASE_URL",
        default_value = "host=localhost port=5433 user=ledger password=ledger dbname=ledger"
    )]
    postgres_url: String,

    /// Start without Postgres (DuckDB only) — useful when the container is not up.
    #[arg(long)]
    no_postgres: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "ledger=info,tower_http=info".into()))
        .init();

    let cli = Cli::parse();
    let registry = build_registry(&cli).await?;
    let state = AppState { registry };

    let listener = tokio::net::TcpListener::bind(&cli.addr)
        .await
        .with_context(|| format!("bind {}", cli.addr))?;
    tracing::info!(addr = %cli.addr, "ledger listening");

    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;
    Ok(())
}

/// Assemble the engines the server will expose. Order here is display order.
async fn build_registry(cli: &Cli) -> anyhow::Result<Registry> {
    let mut engines: Vec<Arc<dyn Engine>> = Vec::new();

    if !cli.no_postgres {
        match PostgresEngine::connect(&cli.postgres_url).await {
            Ok(pg) => engines.push(Arc::new(pg)),
            Err(e) => {
                // Don't hard-fail: a user may want to explore DuckDB alone.
                tracing::warn!(error = %format!("{e:#}"), "Postgres unavailable — starting without it. Run `docker compose up -d` to enable it.");
            }
        }
    }

    let duck = DuckDbEngine::in_memory(DUCKDB_SEED).context("initialize DuckDB")?;
    engines.push(Arc::new(duck));

    tracing::info!(count = engines.len(), "engines ready");
    Ok(Registry::new(engines))
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}
