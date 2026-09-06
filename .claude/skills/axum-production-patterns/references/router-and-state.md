# Router composition & shared state

## Composing the router: nest, merge, and the infra/api split

Build the router as a tree, not a flat list. Two combinators do the work:

- **`.nest(prefix, sub_router)`** — mount a sub-router under a path prefix. Each
  feature area owns a `router()` function returning its own `Router`.
- **`.merge(other)`** — fold another router's routes into this one at the same level.
  Use it to combine independently-built subtrees (infra + api + docs).

The house pattern splits routes into **two groups with different middleware**, because
infrastructure and application traffic have opposite needs:

```rust
pub fn build_router(state: AppState) -> Router {
    // Infra routes: liveness/readiness/metrics. NO rate limit, NO timeout —
    // a k8s probe must succeed even when the app is saturated, otherwise the
    // orchestrator kills a pod that is merely busy.
    let infra = Router::new()
        .route("/health/live", get(routes::health::liveness))
        .route("/health/ready", get(routes::health::readiness))
        .route("/metrics", get(routes::health::metrics_handler));

    // API routes: rate-limited and timeout-protected. These layers are applied
    // HERE (per group) so they don't also wrap the infra routes above.
    let api = Router::new()
        .nest("/api/data_view", routes::data_view::router(state.clone()))
        .route("/api/warehouse-type", get(routes::meta::warehouse_type_handler))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .fallback_service(spa_service())
        .layer(axum::middleware::from_fn(middleware::global_timeout))
        .layer(axum::middleware::from_fn_with_state(state.clone(), middleware::rate_limit));

    // Merge both groups, then apply the layers that should wrap EVERYTHING.
    Router::new()
        .merge(infra)
        .merge(api)
        .layer(CompressionLayer::new())
        .layer(build_cors_layer())
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn(middleware::metrics_layer))
        .layer(axum::middleware::from_fn(middleware::request_id))
        .layer(axum::middleware::from_fn(middleware::security_headers))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_SIZE))
        .layer(CatchPanicLayer::new())
        .with_state(state)
}
```

Why this shape:
- **Feature routers stay self-contained.** `routes::data_view::router()` knows its own
  paths; `build_router` only knows the prefix. You can move or version a feature by
  changing one `.nest` line.
- **Probes can't be rate-limited or timed out.** This is the single most common
  production mistake — a load spike trips the limiter, health checks start returning
  429/504, and the orchestrator restarts healthy pods, deepening the outage.
- **`.fallback_service(spa_service())`** serves a single-page app's static files for any
  unmatched route, so client-side routing works on refresh.

## AppState: one struct, cloned cheaply

`AppState` holds everything handlers need. It must be `Clone` — Axum clones it per
request. Make the clone cheap: each field is itself an `Arc`-backed handle (a pool, a
`PrometheusHandle`, an `Arc<dyn Trait>`), so cloning bumps refcounts, not data.

```rust
#[derive(Clone)]
pub struct AppState {
    pool: pool::ConnectionPool,          // r2d2 pool — Arc inside
    backend: String,
    metrics_handle: PrometheusHandle,
    circuit_breaker: CircuitBreaker,     // Arc<atomics> inside
    rate_limiter: RateLimiter,
    heavy_rate_limiter: RateLimiter,
}
```

When a field isn't already shareable, wrap it explicitly:

```rust
#[derive(Clone)]
pub struct AppState {
    pub file_client: Arc<dyn BlobStorage>,
    pub db: DBActionsPool,
    pub schema_registry: Arc<SchemaRegistry>,
    pub created_tables: Arc<RwLock<HashSet<String>>>,  // mutable shared set
}
```

**Construct blocking resources off the async runtime.** r2d2 pool creation is synchronous
and may block for seconds while it dials the database — run it on the blocking pool so it
doesn't stall the reactor at startup:

```rust
let state = tokio::task::spawn_blocking(move || AppState::new(&wt, handle, rl, hrl))
    .await
    .map_err(|e| format!("DB pool task panicked: {e}"))?
    .map_err(|e| format!("failed to connect to database (warehouse_type={warehouse_type}): {e}"))?;
```

Order at startup: install the metrics recorder → spawn rate limiters (need a runtime) →
build `AppState` → build router. Each later step depends on the earlier handles.

## Accessing state in handlers

The `State` extractor pulls out whatever you attached with `.with_state`:

```rust
pub async fn readiness(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // ...
}
```

Keep raw resources private. Handlers call **methods on `AppState`**
(`state.blocking_query(sql)`, `state.ping()`) rather than touching the pool directly.
This is where the circuit breaker and metrics get woven in once, for all callers — see
`resilience-and-shutdown.md`.

## Substates with `FromRef` — when one big state is too coarse

If a handler only needs *part* of the state, you don't have to pass the whole thing.
Implement `FromRef` and extract the substate directly. This keeps handler signatures
honest about what they touch and makes them easier to test.

```rust
use axum::extract::{State, FromRef};

#[derive(Clone)]
struct AppState {
    api_state: ApiState,
    db: Pool,
}

#[derive(Clone)]
struct ApiState { /* ... */ }

impl FromRef<AppState> for ApiState {
    fn from_ref(app: &AppState) -> ApiState {
        app.api_state.clone()
    }
}

// This handler asks only for the substate it needs:
async fn api_users(State(api_state): State<ApiState>) { /* ... */ }

// This one still takes the whole thing:
async fn handler(State(state): State<AppState>) { /* ... */ }
```

`FromRef` is also how the `with_state` machinery composes: `State<Pool>` works as long as
`Pool: FromRef<AppState>`. Prefer `State`/`FromRef` over `Extension<Arc<_>>` — `Extension`
is checked at runtime (a missing one is a 500), while `State` is wired at build time.
