//! The `tako` ingest application.
//!
//! Run only as a service, so the only public item is [`run`] — the entry point
//! the binary (`main.rs`) awaits. Everything else is crate-internal: the modules
//! are private (a module declared at the crate root is still reachable crate-wide
//! via `crate::`), so nothing here forms an external API.
mod error;
mod limits;
mod openapi;
mod parse;
mod registry;
mod routes;
mod state;

// Deterministic synthetic-data generator ("faker"). Compiled for unit tests and
// the `testing` CLI, never in a production build.
#[cfg(any(test, feature = "testing"))]
mod fake;

// Test/bench tooling, gated off by default — see `[features].testing` in
// Cargo.toml. Production builds don't compile this module or its dependencies.
#[cfg(feature = "testing")]
mod testing;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use clap::{Parser, Subcommand};
use tower::ServiceBuilder;
use tower_http::timeout::TimeoutLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::routes::{health, table, upload};
use crate::state::AppState;

/// Global request-rate backstop for the load-bearing routes. Returns 429 with a
/// `Retry-After` header when the fixed per-second window is full. This caps the
/// service's total throughput defensively; per-client fairness stays the
/// gateway's job (see the README's Security section). 429s are visible in the
/// RED metrics via the status label, so there is no separate counter here.
async fn rate_limit(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if state.rate_limiter.try_acquire() {
        return next.run(req).await;
    }
    tracing::warn!("global rate limit exceeded");
    (
        StatusCode::TOO_MANY_REQUESTS,
        [("retry-after", "1")],
        Json(serde_json::json!({ "detail": "Too many requests" })),
    )
        .into_response()
}
#[cfg(feature = "testing")]
use crate::testing::{bench, testkit};

#[derive(Parser)]
#[command(name = "tako", about = "Tako data-ingest API server and test tooling")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the ingest API server (this is the default with no subcommand)
    Serve,
    /// Initialise the warehouse: create the `sandpit` schema and the satellite
    /// registry tables (idempotent). Spelling is deliberate.
    InnitDb,
    /// Generate sample data files in multiple formats
    #[cfg(feature = "testing")]
    GenData {
        /// Output directory for generated files
        #[arg(long, default_value_t = testkit::default_output_dir())]
        output_dir: String,

        /// Number of rows to generate
        #[arg(long, default_value_t = 100)]
        rows: usize,
    },
    /// Upload sample files to the running API
    #[cfg(feature = "testing")]
    TestUpload {
        /// Base URL of the Tako API
        #[arg(long, default_value = "http://localhost:3000")]
        base_url: String,

        /// Schema name to use for upload
        #[arg(long, default_value = "sample_data")]
        schema: String,

        /// SQL table name (defaults to file base name)
        #[arg(long)]
        table: Option<String>,
    },
    /// Benchmark the upload endpoint with concurrent requests
    #[cfg(feature = "testing")]
    Benchmark {
        /// Base URL of the Tako API
        #[arg(long, default_value = "http://localhost:3000")]
        base_url: String,

        /// Schema name to use for upload
        #[arg(long, default_value = "sample_data")]
        schema: String,

        /// SQL table name (defaults to file base name)
        #[arg(long)]
        table: Option<String>,

        /// Maximum concurrent requests
        #[arg(long, default_value_t = 10)]
        concurrency: usize,

        /// Total number of requests to send
        #[arg(long, default_value_t = 50)]
        requests: usize,

        /// File format(s) to benchmark
        #[arg(long, default_value = "parquet")]
        format: testkit::BenchFormat,

        /// Number of rows per generated file
        #[arg(long, default_value_t = 20000)]
        rows: usize,

        /// Warmup requests (excluded from stats)
        #[arg(long, default_value_t = 5)]
        warmup: usize,
    },
    /// Micro-benchmark the in-memory parse_data hot path (Criterion)
    #[cfg(feature = "testing")]
    BenchParse,
}

/// Build the application router and wire in shared state.
async fn create_router() -> Result<Router, Box<dyn std::error::Error>> {
    let app_state = AppState::new().await?;

    // `/upload` lives in its own sub-router because it carries its own (longer)
    // per-request timeout plus a streaming body cap. Keeping it apart from the
    // shared global timeout below means a legitimate large upload is bounded only
    // by its own timeout, never cut at the short global one.
    //  - the timeout returns 408 by dropping the handler future; the detached
    //    `spawn_blocking` parse/COPY are not interrupted — a warehouse statement
    //    timeout bounds the COPY itself;
    //  - the body cap rejects an oversized request *while streaming*, before it
    //    is buffered into memory.
    let upload = Router::new().route(
        "/upload",
        post(upload::upload_file).layer(
            ServiceBuilder::new()
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    limits::upload_timeout(),
                ))
                .layer(DefaultBodyLimit::max(limits::MAX_UPLOAD_BODY_SIZE)),
        ),
    );

    // The table routes are quick (a DB query at most), so they share a short,
    // defensive `global_timeout` (GLOBAL_TIMEOUT_SECS, default 30s) — a hung
    // backend returns 504 instead of pinning a request forever. Scoped to this
    // sub-router, so it never double-wraps `/upload` (which has its own timeout).
    let tables = Router::new()
        .route("/table", get(table::list_tables).post(table::create_table))
        .route("/table/{name}", get(table::get_table))
        .layer(axum::middleware::from_fn(
            service_kit::middleware::global_timeout,
        ));

    // Load-bearing routes (uploads + table ops) share the global rate-limit
    // backstop. Infra (health / readiness / metrics) is deliberately exempt, so a
    // flood cannot 429 the probes and trigger restart loops.
    let limited =
        Router::new()
            .merge(upload)
            .merge(tables)
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                rate_limit,
            ));

    let infra = Router::new()
        .route("/health", get(health::health))
        .route("/health/ready", get(health::readiness))
        .route("/instance_health", get(health::instance_health))
        .route("/metrics", get(health::metrics))
        .layer(axum::middleware::from_fn(
            service_kit::middleware::global_timeout,
        ));

    let router = Router::new()
        .merge(limited)
        .merge(infra)
        // Serve the OpenAPI spec + Swagger UI (shared mount from service-kit).
        .merge(service_kit::openapi::swagger_ui(openapi::api_doc()))
        // Shared middleware, applied to every route. The last `.layer` is
        // outermost, so the effective order is:
        //   catch_panic → compression → security_headers → request_id → metrics_layer → trace → (per-route) → handler
        // - trace_layer: per-request span with status + latency, nested inside
        //   request_id so the id propagates into its events.
        // - metrics_layer: RED metrics into the recorder served at `/metrics`.
        // - request_id: reuses an incoming `x-request-id` (e.g. from the gateway).
        // - security_headers: the standard hardening headers.
        // - compression: gzip the response body, but only above the size gate so
        //   tako's tiny JSON stays uncompressed; it earns its keep on the larger
        //   responses (a growing `/table` list, `/metrics`, the Swagger assets)
        //   and only when the client advertises `Accept-Encoding`.
        // - catch_panic: outermost, so a panic anywhere below becomes a clean 500
        //   instead of dropping the connection (tako parses untrusted file bytes).
        .layer(service_kit::layers::trace_layer())
        .layer(axum::middleware::from_fn(
            service_kit::middleware::metrics_layer,
        ))
        .layer(axum::middleware::from_fn(
            service_kit::middleware::request_id,
        ))
        .layer(axum::middleware::from_fn(
            service_kit::middleware::security_headers,
        ))
        .layer(service_kit::layers::compression_layer(
            service_kit::layers::DEFAULT_COMPRESSION_MIN_SIZE,
        ))
        .layer(service_kit::layers::catch_panic_layer())
        .with_state(app_state);

    Ok(router)
}

/// Install the tracing subscriber from `RUST_LOG`. Shared by `serve` and the
/// one-shot CLI commands so their logs are visible with the same filtering.
fn init_tracing() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tako=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Run the ingest API server until a shutdown signal arrives.
async fn serve() -> anyhow::Result<()> {
    init_tracing();

    limits::log_startup_summary();

    let app = create_router()
        .await
        .map_err(|e| anyhow::anyhow!("failed to create router: {e}"))?;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;

    tracing::info!("Server running on http://0.0.0.0:3000");

    axum::serve(listener, app)
        .with_graceful_shutdown(service_kit::shutdown_signal())
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

/// Parse the CLI and dispatch. The binary (`main.rs`) just awaits this.
pub async fn run() -> anyhow::Result<()> {
    match Cli::parse().command.unwrap_or(Command::Serve) {
        Command::Serve => serve().await?,
        Command::InnitDb => {
            init_tracing();
            registry::init_db().await?;
        }
        #[cfg(feature = "testing")]
        Command::GenData { output_dir, rows } => testkit::gen_data(&output_dir, rows)?,
        #[cfg(feature = "testing")]
        Command::TestUpload {
            base_url,
            schema,
            table,
        } => testkit::test_upload(&base_url, &schema, table.as_deref()).await?,
        #[cfg(feature = "testing")]
        Command::Benchmark {
            base_url,
            schema,
            table,
            concurrency,
            requests,
            format,
            rows,
            warmup,
        } => {
            testkit::run_benchmark(
                &base_url,
                &schema,
                table.as_deref(),
                concurrency,
                requests,
                &format,
                rows,
                warmup,
            )
            .await?
        }
        #[cfg(feature = "testing")]
        Command::BenchParse => bench::run(),
    }

    Ok(())
}
