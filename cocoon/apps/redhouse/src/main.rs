mod app;
mod cli;
mod openapi;
mod routes;

use std::time::Duration;

use service_kit::{AppState, RateLimiter};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Initialise structured tracing with a composable layer stack.
///
/// - `RUST_LOG` controls verbosity (default: `data_view=info,tower_http=info`)
/// - `RUST_ENV=production` switches to JSON output for log aggregation
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Include the DB layers so connection/pool logs are visible by default —
        // the connector lives in `database`/`service_kit`, not `redhouse`.
        EnvFilter::new("redhouse=info,service_kit=info,database=info,tower_http=info")
    });

    let is_production = std::env::var("RUST_ENV")
        .map(|v| v == "production")
        .unwrap_or(false);

    let fmt_layer = if is_production {
        tracing_subscriber::fmt::layer().json().boxed()
    } else {
        tracing_subscriber::fmt::layer().pretty().boxed()
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}

/// Boot the API server: tracing, metrics, DB pool, router, then serve with
/// graceful shutdown. Invoked by the `serve` command (the default); the frontend
/// and dev orchestration live in the other CLI commands (`src/cli.rs`).
async fn start_server() {
    init_tracing();

    // Install the global Prometheus metrics recorder.
    let metrics_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus metrics recorder");

    // Create rate limiters before AppState (requires tokio runtime).
    let global_rps = std::env::var("RATE_LIMIT_RPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100u64);
    let heavy_rps = std::env::var("RATE_LIMIT_HEAVY_RPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10u64);

    let rate_limiter = RateLimiter::new(global_rps, Duration::from_secs(1));
    let heavy_rate_limiter = RateLimiter::new(heavy_rps, Duration::from_secs(1));

    // Redshift-only app: the backend is fixed at compile time, not read from
    // DATA_WAREHOUSE_TYPE.
    let warehouse_type = "amazon_redshift".to_string();
    tracing::info!("Connecting to database (warehouse_type={warehouse_type})...");

    // r2d2 pool creation + initial connections are synchronous.
    // Run on the blocking thread pool to avoid starving the async runtime.
    let wt = warehouse_type.clone();
    let handle = metrics_handle;
    let rl = rate_limiter.clone();
    let hrl = heavy_rate_limiter.clone();
    let state = match tokio::task::spawn_blocking(move || AppState::new(&wt, handle, rl, hrl))
        .await
        .expect("Blocking task panicked")
    {
        Ok(state) => state,
        Err(e) => {
            tracing::error!(error = %e, warehouse_type, "Failed to connect to database — aborting");
            std::process::exit(1);
        }
    };
    // AppState::new already ran a one-shot connectivity check, so we're connected.

    let app = app::build_router(state);

    let addr = "0.0.0.0:8050";
    tracing::info!("Listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app)
        .with_graceful_shutdown(service_kit::shutdown_signal())
        .await
        .expect("Server error");

    tracing::info!("Server shut down gracefully");
}

/// Entry point: parse the CLI, then dispatch the requested command. `serve`
/// spins up the tokio runtime; the other commands run synchronously (deploy uses
/// a blocking HTTP client, which must not run inside an async context).
fn main() {
    use clap::Parser;

    let cli = cli::Cli::parse();
    match cli.command {
        cli::CliCommand::Serve => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            runtime.block_on(start_server());
        }
        command => std::process::exit(cli::dispatch(command)),
    }
}
