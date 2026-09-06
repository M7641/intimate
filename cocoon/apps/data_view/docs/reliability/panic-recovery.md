# Panic Recovery

> Status: **Implemented** — `src/app.rs` via `CatchPanicLayer`

## What

A middleware layer that catches Rust panics in any handler or downstream middleware and converts them into a 500 Internal Server Error response instead of silently dropping the connection.

## Why

In Rust, a panic in an async task (like an Axum handler) **aborts that task** but does not crash the process. The HTTP connection is dropped without a response. From the client's perspective:

| Without CatchPanicLayer                             | With CatchPanicLayer                                  |
| --------------------------------------------------- | ----------------------------------------------------- |
| Connection reset by peer (no HTTP response)         | `500 Internal Server Error` with a body               |
| Client retries blindly — may trigger the same panic | Client sees a clear error, can report it              |
| No log entry (the task died before logging)         | Panic message is logged                               |
| Monitoring sees a "timeout" not an "error"          | Monitoring sees a 500 — counted in error rate metrics |

Panics are meant to be rare in Rust, but they happen — integer overflow in debug mode, `.unwrap()` on an unexpected `None`, index out of bounds from untrusted input. In a web service, a panic should never be silent.

## How — This Repo

```rust
// src/app.rs
use tower_http::catch_panic::CatchPanicLayer;

Router::new()
    // ... routes and other layers ...
    .layer(CatchPanicLayer::new())  // outermost — catches panics from everything
```

`CatchPanicLayer` is placed as the **outermost** layer so it wraps all other middleware and handlers. This ensures:

1. A panic in any handler → caught
2. A panic in the metrics middleware → caught
3. A panic in a custom extractor → caught

### Why outermost?

If CatchPanicLayer were innermost, a panic in an outer middleware would bypass it. Outermost = maximum coverage.

### Production consideration

CatchPanicLayer uses `std::panic::catch_unwind`, which only catches "unwind" panics (the default). If the code sets `panic = "abort"` in Cargo.toml, panics terminate the process immediately and cannot be caught. This repo uses the default `unwind` strategy.
