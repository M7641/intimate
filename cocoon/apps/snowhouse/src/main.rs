mod app;
mod cache;
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
        // the connector lives in `database`/`service_kit`, not `snowhouse`.
        EnvFilter::new("snowhouse=info,service_kit=info,database=info,tower_http=info")
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

    let state = connect_app_state().await;

    // The analytics endpoints read query history exclusively from the owned cache
    // table (Phase 2 — no information_schema fallback), so resolve and verify it
    // before serving a single request.
    init_history_source(&state).await;

    // Keep the query-history cache warm from inside the server, for a deployment
    // without an external scheduler (on by default — hourly; disable by setting
    // SNOWHOUSE_CACHE_AUTO_REFRESH=false).
    cache::spawn_auto_refresh(state.clone());

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

/// Install the metrics recorder, build rate limiters, and connect the DB pool —
/// the shared bring-up for any command that needs a live [`AppState`] (`serve`,
/// `cache`). Exits the process on a fatal connection error, surfacing the real
/// cause instead of letting the pool retry-loop.
async fn connect_app_state() -> AppState {
    // snowhouse only reads metadata and query history — it never runs compute —
    // so it always runs on the smallest warehouse the role can see. This flips
    // the connector's selection on for the whole process, overriding whatever
    // warehouse the environment or the Nimbus connection would otherwise use.
    database::enable_smallest_warehouse();

    // The Prometheus recorder can only be installed once per process; each CLI
    // invocation runs exactly one command, so this is always the first install.
    let metrics_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus metrics recorder");

    // Rate limiters must be created inside the tokio runtime.
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

    // Snowflake-only app: the backend is fixed at compile time, not read from
    // DATA_WAREHOUSE_TYPE.
    let warehouse_type = "snowflake".to_string();
    tracing::info!("Connecting to database (warehouse_type={warehouse_type})...");

    // r2d2 pool creation + the initial connectivity probe are synchronous; run
    // them on the blocking pool so they don't starve the async runtime.
    let wt = warehouse_type.clone();
    match tokio::task::spawn_blocking(move || {
        AppState::new(&wt, metrics_handle, rate_limiter, heavy_rate_limiter)
    })
    .await
    .expect("Blocking task panicked")
    {
        Ok(state) => state,
        Err(e) => {
            tracing::error!(error = %e, warehouse_type, "Failed to connect to database — aborting");
            std::process::exit(1);
        }
    }
}

/// Resolve, verify, and publish the query-history cache table the analytics
/// endpoints read from.
///
/// Phase 2 reads history exclusively from this table — there is deliberately no
/// `information_schema` fallback — so a missing or unset table is a hard,
/// fail-at-boot error rather than a per-request 500. We prove the table is
/// readable *now* with a cheap probe, so the failure surfaces here (pointing at
/// `snowhouse cache init`) instead of on the first user request.
async fn init_history_source(state: &AppState) {
    let table = cache::CACHE_TABLE;
    if let Err(e) = state
        .blocking_query_uncached(format!("SELECT 1 FROM {table} LIMIT 1"))
        .await
    {
        tracing::error!(
            error = %e,
            table,
            "Query-history cache table is not readable — run `snowhouse cache init` (then `backfill`)"
        );
        std::process::exit(1);
    }
    tracing::info!(
        table,
        "Analytics endpoints read from the query-history cache"
    );
    routes::analytics::set_history_source(table.to_string());
}

/// Run a `cache` subcommand: bring up tracing + a live connection, then dispatch.
/// Returns a process exit code.
async fn run_cache(action: cli::CacheAction) -> i32 {
    init_tracing();

    let state = connect_app_state().await;
    match cache::run(&state, action).await {
        Ok(()) => 0,
        Err(e) => {
            tracing::error!(error = %e, "cache command failed");
            1
        }
    }
}

/// Entry point: parse the CLI, then dispatch the requested command. `serve` and
/// `cache` spin up the tokio runtime; the other commands run synchronously
/// (deploy uses a blocking HTTP client, which must not run inside an async
/// context).
fn main() {
    use clap::Parser;

    let cli = cli::Cli::parse();
    match cli.command {
        cli::CliCommand::Serve => {
            block_on_runtime(start_server());
        }
        cli::CliCommand::Cache { action } => {
            std::process::exit(block_on_runtime(run_cache(action)));
        }
        command => std::process::exit(cli::dispatch(command)),
    }
}

/// Build a multi-threaded tokio runtime and drive `future` to completion.
fn block_on_runtime<F: std::future::Future>(future: F) -> F::Output {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");
    runtime.block_on(future)
}
