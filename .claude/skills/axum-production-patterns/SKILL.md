---
name: axum-production-patterns
description: >
  Build production-grade HTTP services in Rust with Axum (0.8) — the patterns
  that turn a toy router into something safe to run under load: app/router
  composition, shared AppState, a central error type that maps to clean JSON,
  the full tower-http + custom middleware stack in the correct order,
  observability (tracing, RED metrics, request IDs, OpenTelemetry), a pooled
  database with a circuit breaker, graceful shutdown, liveness/readiness probes,
  rate limiting and timeouts, OpenAPI, and integration tests. Use this whenever
  the user is building or hardening an Axum service: "set up an Axum app",
  "add a route/handler/extractor", "share state across handlers", "central error
  handling for my API", "add middleware / a tower layer", "CORS / compression /
  request IDs / timeouts", "make this Axum service production-ready", "graceful
  shutdown", "health and readiness endpoints", "structured logging / tracing /
  metrics for my API", "rate limit my endpoints", "connection pool", "test my
  Axum routes", or "OpenAPI / Swagger for Axum". Trigger even when the user just
  says "build an API in Rust", "harden my web service", or pastes an Axum
  handler and asks to improve it. In the cocoon monorepo, reuse the existing
  `service-kit` crate instead of re-implementing these patterns. For testing
  depth defer to rust-testing-standards; for rebuilding a captured API defer to
  reflect-to-axum.
---

# Production-grade Axum services

Axum gives you a router, extractors, and `IntoResponse` — and stops there. Everything
that makes a service *safe to run under load* is something you assemble on top: a
single error type, a deliberate middleware order, observability, a bounded database
pool, graceful shutdown, and probes. This skill is that assembly, drawn from the
house conventions in `cocoon/crates/service-kit` and `cocoon/apps/data_view`.

## First: don't re-implement what already exists

**In the `cocoon` monorepo, reuse `service-kit`.** It already provides `AppState`
(pool + metrics + circuit breaker + rate limiters), `AppError` with `IntoResponse`,
and the middleware stack (`request_id`, `metrics_layer`, timeouts, `rate_limit`,
`security_headers`). Add a dependency on it and call its builders — do not copy these
patterns inline. Read `crates/service-kit/src/{state,error,middleware,pool}.rs` to see
what's available before writing anything new.

Use the patterns below when you're **outside** that monorepo, or extending
`service-kit` itself. They describe what `service-kit` does and why, so you can
reproduce it faithfully elsewhere.

## The production checklist

A service is "production-grade" when each of these has a deliberate answer. Treat it
as the definition of done — not every box must be ticked for an internal tool, but
each *skipped* box should be a decision, not an oversight.

- [ ] **Structure** — `lib.rs` + `main.rs` split so integration tests can import types
- [ ] **State** — one `AppState` (`Clone`), shared via `State<_>`; substates via `FromRef`
- [ ] **Errors** — one error enum implementing `IntoResponse`; safe JSON body; internals logged, not leaked
- [ ] **Middleware** — tower-http + custom layers applied in the right order (see below)
- [ ] **Body limit** — `DefaultBodyLimit` set; oversized requests rejected
- [ ] **Timeouts** — every request bounded; slow handlers return 504, not hang
- [ ] **Rate limiting** — abuse-bounded; infra routes (health/metrics) **exempt**
- [ ] **Panics** — `CatchPanicLayer` turns a panicking handler into a 500, not a dropped connection
- [ ] **Observability** — structured tracing (JSON in prod), RED metrics, a request ID per request
- [ ] **Database** — bounded pool; blocking drivers run on `spawn_blocking`; a circuit breaker fails fast when the DB is down
- [ ] **Shutdown** — SIGTERM/Ctrl-C drains in-flight requests before exit
- [ ] **Probes** — `/health/live` (no deps) and `/health/ready` (pings deps)
- [ ] **CORS** — explicit allowed origins from config, never `*` with credentials
- [ ] **Docs** — OpenAPI via utoipa if the API is consumed by others
- [ ] **Tests** — integration tests exercise the real router/binary over HTTP

## Minimal skeleton

The shape every service shares. `app.rs` builds the router (pure, testable);
`main.rs` owns process concerns (config, tracing, listener, shutdown).

```rust
// app.rs — pure router construction, no I/O. This is what tests call.
pub fn build_router(state: AppState) -> Router {
    // Infra routes are merged WITHOUT rate-limit/timeout so k8s probes
    // never fail just because the app is busy.
    let infra = Router::new()
        .route("/health/live", get(health::liveness))
        .route("/health/ready", get(health::readiness))
        .route("/metrics", get(health::metrics_handler));

    let api = Router::new()
        .nest("/api/things", things::router())
        .layer(from_fn(middleware::global_timeout))
        .layer(from_fn_with_state(state.clone(), middleware::rate_limit));

    Router::new()
        .merge(infra)
        .merge(api)
        // Outermost-first: see "Middleware order" below.
        .layer(CompressionLayer::new())
        .layer(build_cors_layer())
        .layer(TraceLayer::new_for_http())
        .layer(from_fn(middleware::request_id))
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        .layer(CatchPanicLayer::new())
        .with_state(state)
}
```

```rust
// main.rs — process lifecycle.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();                                   // structured logs first
    let state = AppState::new().await?;               // pools, clients, config
    let app = build_router(state);
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())    // drain on SIGTERM
        .await?;
    Ok(())
}
```

The `lib.rs` + `main.rs` split matters: integration tests `use my_service::{build_router, AppState}`
and exercise the router with `tower::ServiceExt::oneshot` — no port to bind, no process to spawn.

## Middleware order — the rule that bites

`.layer()` wraps from the **inside out**: the **last** `.layer()` you call is the
**outermost** — it sees the request **first** and the response **last**. So order
matters, and getting it wrong is a common production bug.

Put cross-cutting safety on the outside so it covers everything beneath it:

```
CatchPanic → BodyLimit → RequestId → Trace → CORS → Compression → [rate-limit, timeout] → Handler
   (outer, runs first)                                                          (inner, runs last)
```

Reasoning, outside-in: catch panics from *all* inner layers; reject oversized bodies
before doing any work; assign a request ID so every downstream log carries it; trace
the fully-decorated request; handle CORS preflight before app logic. Rate-limit and
timeout sit closest to the handler (applied per route group) so infra routes can opt out.

Two ways to express order — pick one and be consistent:
- **Repeated `.layer()`** (house style in `service-kit`): reads bottom-to-top as outer-to-inner.
- **`ServiceBuilder::new().layer(a).layer(b)`** then one `.layer(stack)`: reads top-to-bottom as outer-to-inner. Easier to reason about for long stacks.

## Reference files — read the one you need

Each covers one concern in depth, with faithful code from the house implementation.
Read the relevant file before writing that part; don't reconstruct from memory.

| When you're working on… | Read |
|---|---|
| Router composition, `AppState`, `FromRef` substates, the nest/merge split | `references/router-and-state.md` |
| The error enum, `IntoResponse`, the JSON envelope, leak-safe DB errors | `references/error-handling.md` |
| The full middleware stack + custom `request_id`/`metrics`/`timeout`/`rate_limit`/`security_headers` | `references/middleware.md` |
| tracing setup, RED metrics, request IDs, OpenTelemetry, Prometheus endpoint | `references/observability.md` |
| Connection pool, `spawn_blocking`, circuit breaker, graceful shutdown, health probes | `references/resilience-and-shutdown.md` |
| Integration tests (`oneshot` + testcontainers), OpenAPI/utoipa | `references/testing-and-openapi.md` |

## Axum 0.8 gotchas (from hard experience)

- **Path params use `{param}`**, not the old `:param` — `route("/data/{table}", ...)`.
  Using `:table` compiles but never matches.
- **`async fn main` returning `Result`** needs `#[tokio::main]`; the trailing `Ok(())`
  is easy to forget.
- **Blocking DB drivers** (`postgres`, `duckdb`, r2d2 `get()`) must run inside
  `tokio::task::spawn_blocking` — calling them directly starves the async runtime and
  stalls *every* connection, not just the slow one.
- **`from_fn_with_state` vs `from_fn`**: use the `_with_state` variant when your
  middleware needs `AppState` (e.g. rate limiting reads a shared limiter).
- **`Extension` is a runtime contract**: a missing extension is a 500 at request time,
  not a compile error. Prefer `State`/`FromRef` for anything you can.
- **Handler arg order**: extractors that consume the body (`Json`, `Multipart`,
  `String`) must come **last** — only one body-consuming extractor per handler.
