# OpenTelemetry Fundamentals

Core concepts you need before instrumenting any service.

## The Three Signals

OpenTelemetry unifies three kinds of telemetry data:

| Signal | What it captures | Monorepo today | OTel equivalent |
|--------|-----------------|----------------|-----------------|
| **Traces** | Request flow through services | Nothing | `Span` trees with `trace_id` |
| **Metrics** | Numeric measurements over time | `red_db` Prometheus counters | OTel `Meter` + Prometheus exporter |
| **Logs** | Discrete events with context | `NimbusLogger` (ANSI stdout), `tracing::info!` | Log records correlated to spans via `trace_id` |

Today these signals are completely siloed. A `NimbusLogger` message in `red_db` has no connection to a Prometheus counter for the same request. OTel links them by embedding `trace_id` and `span_id` into every signal.

## Data Model

### Resource

Describes *where* telemetry comes from. Set once at startup, attached to every span/metric/log.

```
Resource {
    service.name:            "flow"
    service.version:         "0.1.0"
    deployment.environment:  "staging"
    host.name:               "flow-pod-abc123"
}
```

### Span

The fundamental unit of tracing — a named, timed operation with structured context:

```
Span {
    trace_id:    "4bf92f3577b34da6a3ce929d0e0e4736"   ← shared by all spans in this trace
    span_id:     "00f067aa0ba902b7"                     ← unique to this span
    parent_id:   "root" or another span_id              ← forms the tree
    name:        "POST /streams/{stream_id}/events"
    kind:        SERVER
    start_time:  2024-01-15T10:30:00.000Z
    end_time:    2024-01-15T10:30:00.045Z
    status:      OK
    attributes: {
        http.method:      "POST"
        http.route:       "/streams/{stream_id}/events"
        http.status_code: 201
        stream_id:        "orders"
    }
}
```

### Span Tree

When a request flows through multiple services, spans nest into a tree:

```
[trace_id: 4bf92f3577b34da6]

red_db: POST /query                          ─── 200ms ──────────────────────┐
  ├── red_db: validate_request               ── 5ms ──┐                      │
  ├── red_db: httpx.request → tako           ──────── 150ms ────────────┐    │
  │     └── tako: POST /upload               ──────── 140ms ──────────┐│    │
  │           ├── tako: parse_multipart      ── 20ms ──┐              ││    │
  │           ├── tako: validate_schema      ── 10ms ──┐              ││    │
  │           └── tako: s3_upload            ── 100ms ─────────┐     ││    │
  └── red_db: format_response               ── 3ms ─┐                │     │
```

Each indented span is a child. The trace ID (`4bf92f...`) is shared across both `red_db` (Python) and `tako` (Rust). This is what distributed tracing gives you — one view of the full request lifecycle.

## W3C Trace Context

The `traceparent` header propagates trace identity across HTTP service boundaries:

```
traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01
             ──  ────────────────────────────────  ────────────────  ──
             │              │                           │             │
          version      trace-id (32 hex)          parent-id       flags
          (always       128-bit, shared            (16 hex)       01 = sampled
           "00")        across all spans           this span's    00 = not sampled
                        in the trace               unique ID
```

**How it flows:**

1. `red_db` receives a request with no `traceparent` → OTel SDK generates a new `trace_id`
2. `red_db` calls `tako` via `httpx` → SDK auto-injects `traceparent` header with the same `trace_id` but a new `parent-id`
3. `tako` extracts the header → creates a child span under the same trace
4. Both services' spans share the trace ID → Tempo stitches them into one tree

## SDK vs API Separation

OTel splits into two layers:

```
┌─────────────────────────┐
│    Your Application      │
│    (import otel API)     │  ← API: lightweight, safe to depend on everywhere
├─────────────────────────┤
│    OTel SDK              │  ← SDK: heavy, configured once at startup
│    (TracerProvider,      │     (sets up exporters, processors, samplers)
│     BatchSpanProcessor,  │
│     OTLP Exporter)       │
└─────────────────────────┘
```

- **API** (`opentelemetry-api` in Python, `opentelemetry` crate in Rust): defines the `Tracer`, `Span`, and `Meter` interfaces. Library code depends only on this. If no SDK is configured, API calls are safe no-ops.
- **SDK** (`opentelemetry-sdk` / `opentelemetry_sdk`): the implementation — span processing, export, sampling. Configured once in your application's entry point.

This is analogous to how the monorepo already works with `tracing` in Rust:
- `tracing` crate = API (macros like `tracing::info!`)
- `tracing-subscriber` = SDK (the `fmt` layer, `EnvFilter`, etc.)

## Mapping to What Exists

| OTel concept | Rust (`tracing` crate) | Python (`logging` stdlib) |
|-------------|----------------------|--------------------------|
| Span | `tracing::Span` / `#[instrument]` | Manual (no equivalent without OTel) |
| Trace ID | Not present | Not present |
| Log record | `tracing::info!("msg", field = val)` | `logging.info("msg")` |
| Resource | Not present | Not present |
| Exporter | `fmt::Layer` → stdout | `StreamHandler` → stdout |

The Rust side is closer to OTel-ready because `tracing` spans already have the right shape. Adding `tracing-opentelemetry` bridges them to OTel without replacing any existing code. The Python side requires explicit OTel SDK setup since `logging` has no span concept.

## Key Principle: OTel is Additive

Nothing in the existing stack needs to be removed to add OTel:

- `tracing_subscriber::fmt` keeps working → you add an `OpenTelemetryLayer` alongside it
- `NimbusLogger` keeps working → you add `opentelemetry-instrumentation-logging` to inject `trace_id` into log records
- `prometheus-fastapi-instrumentator` keeps working → OTel metrics run in parallel

The [migration guide](07-migration.md) phases out legacy tooling only after OTel equivalents are validated.
