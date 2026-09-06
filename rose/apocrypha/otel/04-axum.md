# Axum Instrumentation

Adding OpenTelemetry traces and metrics to Rust/Axum services using the `tracing` ecosystem.

**Key insight:** Rust's `tracing` crate already produces spans. The `tracing-opentelemetry` crate bridges them to OTel — no replacing existing `tracing::info!` calls or `#[instrument]` attributes. You add a new layer to the existing subscriber stack.

## Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
# Existing tracing deps (keep these)
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# New: OpenTelemetry bridge + SDK + OTLP exporter
tracing-opentelemetry = "0.29"
opentelemetry = "0.28"
opentelemetry_sdk = { version = "0.28", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.28", features = ["tonic"] }

# New: HTTP tracing middleware (auto-creates spans for every request)
tower-http = { version = "0.6", features = ["trace", "cors"] }  # add "trace" feature
```

## Layer Architecture

The `tracing-subscriber` registry composes layers. Each layer independently processes spans/events:

```
┌──────────────────────────────────────────────┐
│              tracing::info!(...)              │
│              #[tracing::instrument]           │
└───────────────────┬──────────────────────────┘
                    │
                    ▼
┌──────────────────────────────────────────────┐
│           tracing_subscriber::Registry       │
│                                              │
│  ┌────────────────────────────────────────┐  │
│  │  EnvFilter layer                       │  │  ← controls log levels (RUST_LOG)
│  ├────────────────────────────────────────┤  │
│  │  fmt::Layer                            │  │  ← console output (keep this!)
│  ├────────────────────────────────────────┤  │
│  │  OpenTelemetryLayer         ← NEW      │  │  ← exports spans to OTel Collector
│  └────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

The `fmt::Layer` stays — you still get console logs during development. The `OpenTelemetryLayer` runs in parallel, sending spans to the Collector via OTLP/gRPC.

## Setup Function

```rust
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{
    trace::{SdkTracerProvider, Tracer},
    Resource,
};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn init_otel_layer(service_name: &str) -> (OpenTelemetryLayer<tracing_subscriber::Registry, Tracer>, SdkTracerProvider) {
    let exporter = SpanExporter::builder()
        .with_tonic()        // gRPC — reads OTEL_EXPORTER_OTLP_ENDPOINT (default: http://localhost:4317)
        .build()
        .expect("failed to create OTLP exporter");

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            Resource::builder()
                .with_service_name(service_name)
                .build(),
        )
        .build();

    let tracer = provider.tracer("tracing-otel");
    let layer = OpenTelemetryLayer::new(tracer);

    (layer, provider)
}
```

The function returns both the layer (to compose into the subscriber) and the provider (to shut down gracefully).

## Service Integration Patterns

Each Axum service has a different starting point. Here's how to integrate OTel into each:

### `flow` — Replace `fmt().init()` with Layered Registry

**Current** (`rose/flow/src/main.rs`):
```rust
tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "flow=info".into()))
    .init();
```

**After:**
```rust
let (otel_layer, otel_provider) = init_otel_layer("flow");

tracing_subscriber::registry()
    .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "flow=info".into()))
    .with(tracing_subscriber::fmt::layer())    // console output (same as before)
    .with(otel_layer)                           // NEW: exports to Collector
    .init();
```

`flow` currently has no graceful shutdown. Add a minimal one to flush spans on exit:

```rust
let app = Router::new()
    // ... routes ...
    .with_state(state);

let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.expect("failed to bind");
tracing::info!("listening on http://0.0.0.0:3000");

axum::serve(listener, app)
    .with_graceful_shutdown(async {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("shutting down");
    })
    .await
    .expect("server error");

// Flush remaining spans before exit
otel_provider.shutdown().expect("failed to shutdown OTel provider");
```

Also add `tower_http::trace::TraceLayer` to the router for automatic per-request spans:

```rust
use tower_http::trace::TraceLayer;

let app = Router::new()
    .route("/streams/{stream_id}/events", post(flow::handlers::append_event))
    .route("/streams/{stream_id}/events", get(flow::handlers::get_stream_events))
    .route("/events/stream", get(flow::handlers::event_stream))
    .route("/health", get(flow::handlers::health))
    .layer(TraceLayer::new_for_http())    // NEW: auto HTTP spans
    .with_state(state);
```

### `tako` — Add OTel Layer to Existing Registry

**Current** (`cocoon/tako/src/main.rs`):
```rust
tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::new(
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tako=debug".into()),
    ))
    .with(tracing_subscriber::fmt::layer())
    .init();
```

**After:**
```rust
let (otel_layer, otel_provider) = init_otel_layer("tako");

tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::new(
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tako=debug".into()),
    ))
    .with(tracing_subscriber::fmt::layer())
    .with(otel_layer)    // NEW: one line added
    .init();
```

`tako` already has graceful shutdown. Add OTel shutdown to the existing sequence:

```rust
// In main(), after axum::serve(...).await:
tracing::info!("Shutting down background workers...");
worker_handle.shutdown();

// NEW: flush remaining spans
if let Err(e) = otel_provider.shutdown() {
    tracing::error!("failed to shutdown OTel provider: {:?}", e);
}

tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
tracing::info!("Server shutdown complete");
```

**Note:** `tako` has `console-subscriber = "0.5"` in Cargo.toml but never initializes it. You can remove it or keep it — it doesn't conflict with the OTel layer.

### `obscur` — Bootstrap from Scratch

**Current** (`rose/obscur/src/main.rs`): Uses `eprintln!` — no `tracing` crate at all.

**Step 1:** Add dependencies to `Cargo.toml`:
```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-opentelemetry = "0.29"
opentelemetry = "0.28"
opentelemetry_sdk = { version = "0.28", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.28", features = ["tonic"] }
tower-http = { version = "0.6", features = ["trace"] }
```

**Step 2:** Replace `eprintln!` with `tracing`:
```rust
// Before:
eprintln!("Loading model (first run downloads from HuggingFace)...");
// After:
tracing::info!("loading model (first run downloads from HuggingFace)");
```

**Step 3:** Initialize the subscriber:
```rust
let (otel_layer, _otel_provider) = init_otel_layer("obscur");

tracing_subscriber::registry()
    .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "obscur=info".into()))
    .with(tracing_subscriber::fmt::layer())
    .with(otel_layer)
    .init();
```

## Handler Instrumentation with `#[instrument]`

Add `#[tracing::instrument]` to Axum handlers to create named spans with captured arguments:

```rust
use tracing::instrument;

#[instrument(
    skip(state),                               // don't serialize AppState
    fields(stream_id = %stream_id),            // include path param as span field
)]
async fn append_event(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
    Json(payload): Json<AppendEventRequest>,
) -> Result<Json<Event>, AppError> {
    // All tracing::info! calls inside here are children of this span
    tracing::info!(event_type = %payload.event_type, "appending event");
    // ...
}
```

**Field guidance:**
- `skip` anything that doesn't implement `Debug` or is too large (State, connection pools)
- Use `%` for Display formatting, `?` for Debug
- Keep field names consistent across services: `stream_id`, `file_name`, `query_id`

## TraceLayer for Automatic HTTP Spans

`tower_http::trace::TraceLayer` creates a span for every request/response cycle:

```rust
use tower_http::trace::TraceLayer;

let app = Router::new()
    .route("/upload", post(upload_handler))
    .layer(TraceLayer::new_for_http())
    .with_state(state);
```

Each request automatically gets a span with `http.method`, `http.uri`, `http.status_code`, and latency. This works alongside `#[instrument]` — the handler span becomes a child of the HTTP span.

## Context Propagation (Incoming Requests)

When a Python service calls a Rust service with a `traceparent` header, the Rust side needs to extract it. With `tracing-opentelemetry` and `TraceLayer`, this happens via a custom extractor:

```rust
use opentelemetry::propagation::TextMapExtractor;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry::global;

// Set the global propagator once at startup (before .init()):
global::set_text_map_propagator(TraceContextPropagator::new());
```

Then use `axum` middleware to extract context from incoming headers:

```rust
use axum::middleware;
use axum::extract::Request;
use opentelemetry::global;
use tracing_opentelemetry::OpenTelemetrySpanExt;

async fn extract_otel_context(
    request: Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(request.headers()))
    });
    tracing::Span::current().set_parent(parent_context);
    next.run(request).await
}

// Simple wrapper to implement TextMapExtractor for HeaderMap
struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

impl opentelemetry::propagation::Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

// Wire it into the router:
let app = Router::new()
    .route("/upload", post(upload_handler))
    .layer(middleware::from_fn(extract_otel_context))
    .layer(TraceLayer::new_for_http())
    .with_state(state);
```

See [05-cross-service.md](05-cross-service.md) for the full Python ↔ Rust propagation walkthrough.

## Configuration via Environment Variables

```bash
OTEL_SERVICE_NAME=flow                              # overrides resource service.name
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317   # gRPC for Rust (tonic)
OTEL_TRACES_SAMPLER=parentbased_traceidratio
OTEL_TRACES_SAMPLER_ARG=1.0                          # 100% in dev
RUST_LOG=flow=debug                                  # existing, still works
```

## Verification

```bash
# 1. Start collector stack
cd apocrypha/otel && docker compose up -d

# 2. Run flow with OTel
cd rose/flow && OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 cargo run

# 3. Make requests
curl -X POST http://localhost:3000/streams/test/events \
  -H "Content-Type: application/json" \
  -d '{"event_type":"test","payload":{}}'

# 4. Check collector logs
docker compose -f apocrypha/otel/docker-compose.yml logs otel-collector

# 5. View traces in Grafana Tempo
open http://localhost:3001/explore
# Tempo → Search → service.name = "flow"
```

## Next Steps

- [05-cross-service.md](05-cross-service.md) — connect Rust ↔ Python traces
- [06-production.md](06-production.md) — sampling, performance, and security
