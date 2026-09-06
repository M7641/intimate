# Observability

You cannot operate what you cannot see. Three pillars, each cheap to add up front and
painful to retrofit during an incident: **structured logs**, **metrics**, and a
**request ID** that ties them together (plus optional distributed tracing via OTel).

## Tracing: structured logs, JSON in production

Build the subscriber as a composable layer stack. The key production move: emit **pretty**
logs locally (human-readable) and **JSON** in production (machine-parseable by your log
aggregator). Switch on an env var.

```rust
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("data_view=info,tower_http=info"));

    let is_production = std::env::var("RUST_ENV").map(|v| v == "production").unwrap_or(false);

    let fmt_layer = if is_production {
        tracing_subscriber::fmt::layer().json().boxed()    // structured for aggregation
    } else {
        tracing_subscriber::fmt::layer().pretty().boxed()  // readable for humans
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}
```

Call `init_tracing()` **first** in `main`, before anything that might log. `RUST_LOG`
(read by `EnvFilter`) controls verbosity per-module: `RUST_LOG=data_view=debug,tower_http=info`.

Use **structured fields**, not string interpolation, so logs are queryable:

```rust
tracing::error!(error.kind = "database", error = %err, table = %name, "Query failed");
//             ^ key=value fields land as JSON keys; the message is the last arg
```

## Request ID: the thread that ties logs together

The `request_id` middleware (see `middleware.md`) opens a span carrying `request_id`, and
`.instrument(span)` makes **every** log emitted while handling that request inherit the
field automatically. One grep by request ID reconstructs the whole request's story across
all the lines it produced. The ID is also returned in the `x-request-id` response header so
a client can quote it.

## Metrics: RED + resource gauges

Use the `metrics` facade with the Prometheus exporter. Install the recorder once at startup,
*before* building `AppState`, and keep the handle to render the endpoint:

```rust
let handle = metrics_exporter_prometheus::PrometheusBuilder::new()
    .install_recorder()
    .expect("install prometheus recorder");
```

Record the **RED** signals in the `metrics_layer` middleware (Rate, Errors via the status
label, Duration) — see `middleware.md`. Add **resource gauges** where the resource lives;
the house pattern reports pool health on every query so you can correlate latency with pool
exhaustion:

```rust
let ps = self.pool.state();
metrics::gauge!("db_pool_connections_total").set(f64::from(ps.connections));
metrics::gauge!("db_pool_connections_idle").set(f64::from(ps.idle_connections));
metrics::gauge!("db_circuit_breaker_state").set(self.circuit_breaker.state_gauge());
metrics::counter!("db_queries_total", "status" => "success").increment(1);
metrics::histogram!("db_query_duration_seconds").record(elapsed.as_secs_f64());
```

Expose them on `/metrics` (an infra route, **not** rate-limited — Prometheus scrapes it
every few seconds):

```rust
pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    state.render_metrics()   // handle.render() -> Prometheus text exposition format
}
```

**Watch cardinality.** Every distinct label-value combination is a separate time series.
Label by *matched route* (`/users/{id}`), method, and status — never by raw path, user ID,
or anything unbounded.

## OpenTelemetry: opt-in distributed tracing

Wire OTel behind a `#[cfg(feature = "otel")]` flag *and* an env var, so it costs nothing
when unused and turns on without a recompile decision in environments that have a collector:

```rust
let (otel_layer, provider) = if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
    match init_otel_provider() {
        Ok(provider) => {
            let tracer = provider.tracer("data-view");
            (Some(tracing_opentelemetry::layer().with_tracer(tracer)), Some(provider))
        }
        Err(e) => { eprintln!("Failed to init OpenTelemetry: {e}"); (None, None) }
    }
} else {
    (None, None)
};
registry.with(otel_layer).init();   // None layer is a no-op
```

The OTel layer plugs into the *same* `tracing` registry — your existing spans become
distributed traces with no change to handler code. **Flush on shutdown** or you lose the
last batch of spans (see `resilience-and-shutdown.md`):

```rust
#[cfg(feature = "otel")]
if let Some(provider) = otel_provider {
    let _ = provider.shutdown();   // flush pending spans before exit
}
```

## Crate versions (workspace)

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
metrics = "0.24"
metrics-exporter-prometheus = "0.16"
# otel feature only:
opentelemetry = "0.28"
opentelemetry_sdk = { version = "0.28", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.28", features = ["grpc-tonic"] }
tracing-opentelemetry = "0.29"
```

The OTel crate versions move in lockstep and break across minor bumps — pin them together
and upgrade as a set.
