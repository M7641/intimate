//! perch — petit serveur HTTP Axum conçu pour tourner derrière Lambda Web Adapter.
//!
//! Trois endpoints :
//!   GET /         → texte plain
//!   GET /healthz  → JSON {status, uptime_seconds}
//!   GET /env      → JSON {region, stage, rust_log, version}
//!
//! Le binaire tourne aussi localement (`cargo run`) sur le port 8080 ;
//! en Lambda, l'adapter détecte le port et proxie les invocations.

use std::time::Instant;

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Clone)]
struct AppState {
    started_at: Instant,
    region: String,
    stage: String,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    uptime_seconds: u64,
}

#[derive(Serialize)]
struct EnvInfo {
    region: String,
    stage: String,
    rust_log: String,
    version: &'static str,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // JSON logs : CloudWatch Logs Insights les indexe automatiquement par champ.
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=info".into()))
        .with(fmt::layer().json().with_target(false))
        .init();

    let state = AppState {
        started_at: Instant::now(),
        region: std::env::var("AWS_REGION").unwrap_or_else(|_| "local".into()),
        stage: std::env::var("PERCH_STAGE").unwrap_or_else(|_| "dev".into()),
    };

    let app = Router::new()
        .route("/", get(hello))
        .route("/healthz", get(healthz))
        .route("/env", get(env_info))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "perch listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn hello() -> &'static str {
    "perch — small bird, fast cold start\n"
}

async fn healthz(State(state): State<AppState>) -> (StatusCode, Json<Health>) {
    let uptime = state.started_at.elapsed().as_secs();
    (
        StatusCode::OK,
        Json(Health {
            status: "ok",
            uptime_seconds: uptime,
        }),
    )
}

async fn env_info(State(state): State<AppState>) -> Json<EnvInfo> {
    Json(EnvInfo {
        region: state.region.clone(),
        stage: state.stage.clone(),
        rust_log: std::env::var("RUST_LOG").unwrap_or_default(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

// SIGTERM est ce que Lambda envoie quand un environnement d'exécution est recyclé.
// Sans graceful shutdown, les requêtes en vol sont coupées net.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("ctrl_c handler");
    };
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("sigterm handler")
            .recv()
            .await;
    };
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
