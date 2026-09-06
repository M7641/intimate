//! Entry point: wire the pieces together, spawn the background digest, serve.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;

use oracle::config::Config;
use oracle::embed::HashEmbedder;
use oracle::llm::auto_client;
use oracle::server::{router, AppState};
use oracle::{curate, db};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "oracle=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env();
    tracing::info!(?config, "starting oracle");

    let pool = db::init_pool(&config.db_path).await?;
    // Ensure a taste profile exists from the first request.
    db::current_profile(&pool).await?;

    let embedder = Arc::new(HashEmbedder::new(config.embed_dim));
    let llm = auto_client();
    let state = AppState::new(pool, config.clone(), embedder, Arc::from(llm));

    // Background digest: periodically judge the pending pool as a batch.
    spawn_digest(state.clone(), config.digest_interval_secs);

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!("listening on http://{}", config.bind);
    tracing::info!("SSE feed: curl -N http://{}/feed", config.bind);

    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// The digest lane's heartbeat — runs `run_digest` on a fixed interval. Prefer a
/// simple fixed interval over anything fancier; it's predictable and easy to tune
/// via `ORACLE_DIGEST_INTERVAL`.
fn spawn_digest(state: AppState, interval_secs: u64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.tick().await; // consume the immediate first tick
        loop {
            ticker.tick().await;
            match curate::run_digest(&state).await {
                Ok(0) => {}
                Ok(n) => tracing::info!("digest published {n} item(s)"),
                Err(e) => tracing::warn!("digest failed: {e}"),
            }
        }
    });
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}
