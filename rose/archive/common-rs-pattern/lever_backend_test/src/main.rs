use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Extension;
use tower_http::compression::{CompressionLayer, CompressionLevel};

use common_rs::db::Db;
use common_rs::db::redshift::RedshiftConfig;
use common_rs::env::EnvManager;
use common_rs::logger::init_tracing;
use common_rs::spa::{self, DistPath};

mod routes;
mod state;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing("info,levers_backend=debug,sqlx::query=error");

    let env = EnvManager::from_env();
    let cfg = RedshiftConfig::from_env()?;
    tracing::info!(target = env.target().as_str(), "starting levers-backend");

    let db = Db::connect(&env, &cfg).await?;
    let app_state = state::AppState::new(db, env);

    let dist = DistPath::resolve(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("backend has parent (levers module dir)")
            .join("frontend")
            .join("dist"),
    );

    let router = routes::assemble_router()
        .with_state(app_state)
        .fallback(spa::fallback)
        .layer(Extension(dist))
        .layer(CompressionLayer::new().quality(CompressionLevel::Best));

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8050);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "levers-backend listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
