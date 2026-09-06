# Middleware Ordering

> Status: **Implemented** — `src/app.rs`

## What

The specific sequence in which middleware layers process requests and responses. In Axum (Tower), the order is critical — it determines what each layer can see, measure, and catch.

## Why

Middleware ordering is not arbitrary. Getting it wrong causes subtle bugs:

| Mistake                     | Consequence                                                                      |
| --------------------------- | -------------------------------------------------------------------------------- |
| Metrics inside CatchPanic   | A panic bypasses metrics → error rate underreported → false sense of reliability |
| Request ID after TraceLayer | TraceLayer's spans don't carry the request_id → logs can't be correlated         |
| Compression outside Metrics | Metrics measures compression time → latency numbers are inflated                 |
| CatchPanic innermost        | A panic in the metrics middleware kills the connection silently                  |

The correct order ensures maximum coverage, accurate measurements, and proper context propagation.

## How — This Repo

### Layer order (`src/app.rs`)

```rust
Router::new()
    // ... routes ...
    .layer(CompressionLayer::new())                             // 1 - innermost
    .layer(TraceLayer::new_for_http())                          // 2
    .layer(axum::middleware::from_fn(middleware::metrics_layer)) // 3
    .layer(axum::middleware::from_fn(middleware::request_id))    // 4
    .layer(CatchPanicLayer::new())                              // 5 - outermost
```

In Axum, each `.layer()` call **wraps** the existing service. The last added is the outermost — the first to execute.

### Execution flow

```
Incoming Request
       │
       ▼
┌──────────────────┐
│  CatchPanicLayer │  Outermost — catches panics from everything below
├──────────────────┤
│  Request ID      │  Generates UUID, creates tracing span
├──────────────────┤
│  RED Metrics     │  Starts timer, records rate/errors/duration
├──────────────────┤
│  TraceLayer      │  Logs HTTP details (within request_id span)
├──────────────────┤
│  Compression     │  Innermost — compresses response body
├──────────────────┤
│     Handler      │  Business logic + DB queries
└──────────────────┘
       │
       ▼
Outgoing Response (compressed, with x-request-id header, metrics recorded)
```

### Why this specific order?

| Position  | Layer           | Rationale                                                                                                                                                                                     |
| --------- | --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Outermost | **CatchPanic**  | Must wrap everything — a panic in any layer is caught and converted to 500                                                                                                                    |
| 2nd       | **Request ID**  | Creates the tracing span that all downstream layers inherit. TraceLayer, Metrics, and handlers all emit logs with `request_id` in context                                                     |
| 3rd       | **Metrics**     | Measures total processing time including TraceLayer but excluding CatchPanic/RequestID overhead (negligible). Counts 500s from panics correctly because CatchPanic converts them to responses |
| 4th       | **TraceLayer**  | HTTP-level logging. Runs inside the request_id span, so logs carry the correlation ID                                                                                                         |
| Innermost | **Compression** | Operates on the response body after all processing. Does not affect timing measurements                                                                                                       |

### Common question: why not measure after compression?

The metrics middleware runs _outside_ compression, so `http_request_duration_seconds` excludes compression time. This is intentional — compression time is CPU overhead, not business logic latency. If you want to measure total time including compression, move the metrics layer inward. But generally, operators care about "how long did the DB query take?" not "how long did gzip encoding take?"
