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

/// Boxed error for the boot sequence: any startup step can fail with a clear,
/// `Display`-able message that `main` logs before exiting, instead of panicking.
type BootError = Box<dyn std::error::Error + Send + Sync>;

/// Initialise structured tracing with a composable layer stack.
///
/// - `RUST_LOG` controls verbosity (default: `data_view=info,tower_http=info`)
/// - `RUST_ENV=production` switches to JSON output for log aggregation
/// - With the `otel` feature AND `OTEL_EXPORTER_OTLP_ENDPOINT` set, adds an
///   OpenTelemetry export layer that converts `tracing` spans into OTel traces.
///
/// Returns the OTel tracer provider (if created) so the caller can `.shutdown()`
/// it on exit.
#[cfg(feature = "otel")]
fn init_tracing() -> Option<opentelemetry_sdk::trace::SdkTracerProvider> {
    use opentelemetry::trace::TracerProvider;

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("data_view=info,tower_http=info"));

    let is_production = std::env::var("RUST_ENV")
        .map(|v| v == "production")
        .unwrap_or(false);

    let fmt_layer = if is_production {
        tracing_subscriber::fmt::layer().json().boxed()
    } else {
        tracing_subscriber::fmt::layer().pretty().boxed()
    };

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    let (otel_layer, provider) = if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
        match init_otel_provider() {
            Ok(provider) => {
                let tracer = provider.tracer("data-view");
                eprintln!("OpenTelemetry tracing enabled");
                (
                    Some(tracing_opentelemetry::layer().with_tracer(tracer)),
                    Some(provider),
                )
            }
            Err(e) => {
                eprintln!("Failed to init OpenTelemetry tracer: {e}");
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    registry.with(otel_layer).init();
    provider
}

#[cfg(not(feature = "otel"))]
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("data_view=info,tower_http=info"));

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

/// Build the OTel OTLP tracer provider (0.28 API).
#[cfg(feature = "otel")]
fn init_otel_provider()
-> Result<opentelemetry_sdk::trace::SdkTracerProvider, Box<dyn std::error::Error>> {
    use opentelemetry_otlp::SpanExporter;
    use opentelemetry_sdk::{Resource, trace::SdkTracerProvider};

    let exporter = SpanExporter::builder().with_tonic().build()?;

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(Resource::builder().with_service_name("data-view").build())
        .build();

    opentelemetry::global::set_tracer_provider(provider.clone());

    Ok(provider)
}

/// Boot the API server: tracing, metrics, DB pool, router, then serve with
/// graceful shutdown. Invoked by the `serve` command (the default); the frontend
/// and dev orchestration live in the other CLI commands (`src/cli.rs`).
///
/// On any startup failure this returns `Err` instead of panicking, so a missing
/// database, bad config, or taken port surfaces as a clean log line + `exit(1)`
/// rather than a backtrace — which matters in local dev, where the server
/// restarts on every hot-reload.
///
/// The DB pool needs no special teardown here: `PostgresDatabase`'s `Drop`
/// closes each sync connection on a runtime-free thread, so the pool can drop
/// on any thread (including this one) without panicking.
async fn start_server() -> Result<(), BootError> {
    #[cfg(feature = "otel")]
    let _otel_provider = init_tracing();
    #[cfg(not(feature = "otel"))]
    init_tracing();

    // Install the global Prometheus metrics recorder.
    let metrics_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| format!("failed to install Prometheus metrics recorder: {e}"))?;

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

    let warehouse_type =
        std::env::var("DATA_WAREHOUSE_TYPE").unwrap_or_else(|_| "amazon_redshift".to_string());
    tracing::info!("Connecting to database (warehouse_type={warehouse_type})...");

    // r2d2 pool creation + initial connections are synchronous.
    // Run on the blocking thread pool to avoid starving the async runtime.
    let wt = warehouse_type.clone();
    let handle = metrics_handle;
    let rl = rate_limiter.clone();
    let hrl = heavy_rate_limiter.clone();
    let state = tokio::task::spawn_blocking(move || AppState::new(&wt, handle, rl, hrl))
        .await
        .map_err(|e| format!("DB pool task panicked: {e}"))?
        .map_err(|e| {
            format!("failed to connect to database (warehouse_type={warehouse_type}): {e}")
        })?;

    let s = state.clone();
    tokio::task::spawn_blocking(move || s.ping())
        .await
        .map_err(|e| format!("DB ping task panicked: {e}"))?
        .map_err(|e| format!("database ping failed: {e}"))?;
    tracing::info!("Database connection verified");

    let app = app::build_router(state);

    // Port is fixed in normal operation; `DATA_VIEW_PORT` overrides it so an
    // integration test can run the real server on a free port.
    let port = std::env::var("DATA_VIEW_PORT").unwrap_or_else(|_| "8050".to_string());
    let addr = format!("0.0.0.0:{port}");
    // `0.0.0.0` is a bind address (all interfaces), not a browsable one.
    tracing::info!("Listening on {addr}");

    // The router serves the built SPA only when a bundle is found (see
    // `app::spa_bundle_dir`). In local dev there's no bundle: the UI is served
    // by Vite and this server only answers `/api`. Point the operator at
    // whichever URL is actually live.
    match app::spa_bundle_dir() {
        Some(dir) => {
            tracing::info!("Serving frontend from {}", dir.display());
            tracing::info!("Open the app at http://localhost:{port}");
        }
        None => tracing::info!(
            "No frontend bundle found — serving API only at http://localhost:{port}. \
             In local dev, open the Vite dev server at http://localhost:5173 instead."
        ),
    }

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("failed to bind {addr}: {e}"))?;

    axum::serve(listener, app)
        .with_graceful_shutdown(service_kit::shutdown_signal())
        .await
        .map_err(|e| format!("server error: {e}"))?;

    // Flush any pending OTel spans before exit.
    #[cfg(feature = "otel")]
    if let Some(provider) = _otel_provider {
        if let Err(e) = provider.shutdown() {
            eprintln!("OTel tracer provider shutdown error: {e}");
        }
    }

    tracing::info!("Server shut down gracefully");
    Ok(())
}

/// Entry point: parse the CLI, then dispatch the requested command. `serve`
/// spins up the tokio runtime; the other commands run synchronously (deploy uses
/// a blocking HTTP client, which must not run inside an async context).
fn main() {
    use clap::Parser;

    let cli = cli::Cli::parse();
    // No subcommand defaults to `serve` — the production entrypoint path.
    match cli.command.unwrap_or(cli::CliCommand::Serve) {
        cli::CliCommand::Serve => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime");
            // Boot failure (no DB, bad env, port taken) surfaces as a plain
            // error line + non-zero exit — no panic, no backtrace.
            if let Err(err) = runtime.block_on(start_server()) {
                tracing::error!("data_view failed to start: {err}");
                std::process::exit(1);
            }
        }
        command => std::process::exit(cli::dispatch(command)),
    }
}
