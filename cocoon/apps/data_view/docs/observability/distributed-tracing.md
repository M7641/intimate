# Distributed Tracing

> Status: **Implemented (feature-gated)** — behind the `otel` cargo feature flag

## What

A trace is a tree of causally-related **spans** that follows a single request through the system. Each span records a unit of work (HTTP handler, DB query, external call) with its start time, duration, and structured metadata.

Distributed tracing extends this across service boundaries by propagating a **trace context** (trace ID + parent span ID) in HTTP headers.

## Why

Logs and metrics tell you that something is slow or failing. Traces tell you **where** and **why**.

| Scenario                       | Without traces                                                               | With traces                                                                |
| ------------------------------ | ---------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| "The `/data` endpoint is slow" | Check logs for `elapsed_ms` — but which layer is slow? Handler? DB? Network? | Waterfall view: handler=2ms, DB query=3400ms — the query is the bottleneck |
| "Errors spike at 14:00"        | Search logs by timestamp — hundreds of lines to correlate                    | Filter by error spans, see that all failed requests hit one specific query |
| "This request timed out"       | `request_id` helps find logs, but you still read lines sequentially          | Visual trace shows the request spent 29s waiting for a pool connection     |

Traces turn debugging from "reading a novel" into "looking at a map".

## How — Current State

The codebase already instruments handlers and queries with `tracing` spans:

```rust
// Handler span (automatic via macro)
#[tracing::instrument(skip(state), fields(table = %table_name))]
pub async fn columns_handler(...) { ... }

// Query span (manual)
tracing::debug!(sql = %sql, "Executing query");
tracing::info!(rows = rows.len(), elapsed_ms, "Query completed");
```

These spans form a tree for each request:

```
request (request_id=abc-123)
  └── columns_handler (table=users)
       └── blocking_query (sql="SELECT ...", elapsed_ms=45)
```

## How — OpenTelemetry Export

OpenTelemetry export is implemented behind the `otel` cargo feature flag so it adds zero overhead when not enabled.

### Building with tracing support

```bash
cargo build --features otel
```

### Activation

The OTel layer is only activated at runtime when the `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable is set:

```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 cargo run --features otel
```

If the feature is compiled in but the env var is absent, the service runs with only the standard `fmt` subscriber layer — no OTel overhead.

### Dependencies (feature-gated)

| Crate | Version | Purpose |
|-------|---------|---------|
| `opentelemetry` | 0.28 | Core OTel API |
| `opentelemetry_sdk` | 0.28 | SDK with batch span processor |
| `opentelemetry-otlp` | 0.28 | OTLP exporter via gRPC (tonic) |
| `tracing-opentelemetry` | 0.29 | Bridge from `tracing` spans to OTel spans |

### Architecture

The `init_tracing()` function in `src/main.rs` uses the registry + layers pattern:

1. **`EnvFilter` layer** — controls log verbosity (always present)
2. **`fmt` layer** — structured JSON logs to stdout (always present)
3. **OTel layer** (conditional) — converts `tracing` spans into OTel spans and exports them via OTLP/gRPC

The OTel layer uses:

- `SdkTracerProvider` with a **batch exporter** (batches spans before sending to reduce network overhead)
- A `Resource` with `service.name = "data-view"` for identification in the trace backend

### Graceful shutdown

On application exit, the tracer provider is explicitly shut down to flush any remaining spans before the process terminates.

### Viewing traces

Exported traces can be viewed in any OTLP-compatible backend:

- Jaeger
- Grafana Tempo
- Datadog APM
- AWS X-Ray (with OTLP collector)
