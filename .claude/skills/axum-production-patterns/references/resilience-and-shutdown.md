# Resilience: pools, circuit breakers, shutdown, probes

These are the patterns that keep a service alive when its dependencies misbehave and let
it die cleanly when asked. They're what separates "runs on my machine" from "survives a
bad afternoon in production."

## Bounded connection pool

An unbounded pool is a denial-of-service waiting to happen: enough concurrent requests open
enough connections to exhaust the database. Bound it, and tune the timeouts from env so ops
can adjust without a redeploy.

```rust
pub(crate) fn build_pool(warehouse_type: &str) -> Result<ConnectionPool, DatabaseError> {
    let manager = DatabaseManager { config: DatabaseConfig::from_env(warehouse_type)? };

    r2d2::Pool::builder()
        .max_size(env_parse_u32("DB_MAX_CONNECTIONS", 3))
        .connection_timeout(Duration::from_secs(env_parse("DB_CONNECTION_TIMEOUT_SECS", 30)))
        .idle_timeout(Some(Duration::from_secs(env_parse("DB_IDLE_TIMEOUT_SECS", 600))))
        .max_lifetime(Some(Duration::from_secs(env_parse("DB_MAX_LIFETIME_SECS", 1800))))
        .test_on_check_out(true)   // validate a connection before handing it out
        .build(manager)
        .map_err(|e| DatabaseError::ConnectionError(e.to_string()))
}
```

`max_lifetime` recycles connections so a server-side idle-killer or a failover can't leave
you holding dead sockets. `test_on_check_out` trades a tiny ping for never handing a handler
a broken connection. (Async stacks use `sqlx`/`deadpool` instead of r2d2 — same knobs:
max size, acquire timeout, idle/lifetime caps.)

## Blocking drivers go on `spawn_blocking`

`postgres`, `duckdb`, and r2d2's `get()` are **synchronous**. Calling them directly on an
async task blocks the whole reactor thread — one slow query stalls _every_ connection on
that thread, not just its own. Always offload to the blocking pool:

```rust
pub async fn blocking_query(&self, sql: String) -> Result<Vec<Row>, AppError> {
    self.circuit_breaker.check()?;                         // fail fast if DB is down
    let pool = self.pool.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;                            // blocking checkout
        conn.query(&sql)                                   // blocking query
    }).await?;                                             // JoinError -> AppError
    // ... record metrics, update circuit breaker ...
    result.map_err(AppError::from)
}
```

This is the single most important async-Rust rule for database work, and the easiest to get
wrong because the blocking call compiles fine inside an `async fn`.

## Circuit breaker: fail fast, recover automatically

When the database is down, you don't want every request to wait the full
`connection_timeout` before failing — that turns a DB outage into total resource exhaustion
as requests pile up. A circuit breaker short-circuits: after N consecutive failures it
"opens" and rejects immediately (503) for a cooldown, then "half-opens" to test recovery.

```rust
self.circuit_breaker.check()?;   // returns AppError::CircuitOpen (-> 503) when open

match result {
    Ok(rows) => { self.circuit_breaker.record_success(); /* ... */ }
    Err(err) => { self.circuit_breaker.record_failure(); /* ... */ }
}
```

States: **closed** (normal) → **open** (failing fast, after `CB_FAILURE_THRESHOLD`
failures) → **half-open** (after `CB_COOLDOWN_SECS`, lets one request through to probe) →
back to closed on success or open on failure. The house implementation is lock-free atomics
(`service-kit/src/circuit_breaker.rs`); export its state as a gauge so you can see it trip.

## Graceful shutdown

On deploy, the orchestrator sends SIGTERM and expects the process to stop accepting new
work, finish in-flight requests, and exit. `axum::serve().with_graceful_shutdown()` does the
draining; you supply a future that resolves when a shutdown signal arrives.

```rust
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(unix)]
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
    #[cfg(not(unix))]
    ctrl_c.await;

    tracing::info!("Shutdown signal received, draining connections...");
}
```

Wire it in and clean up after the server returns:

```rust
axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal())
    .await?;

#[cfg(feature = "otel")]
if let Some(provider) = otel_provider {
    let _ = provider.shutdown();   // flush the last batch of spans
}
tracing::info!("Server shut down gracefully");
```

**SIGTERM is the one that matters** — it's what Kubernetes/Docker send. Handling only
Ctrl-C (SIGINT) means clean shutdown locally but hard kills in production after the grace
period, dropping in-flight requests. The r2d2 pool needs no special teardown: `Drop` closes
each connection.

## Health probes: liveness vs readiness

These answer two different questions, and conflating them causes outages.

**Liveness** — "is the process alive?" No dependency checks. If this fails, the orchestrator
_restarts_ the pod. It must **not** check the database — a DB blip would trigger a restart
storm that can't fix anything.

```rust
pub async fn liveness() -> Json<Value> {
    Json(json!({ "status": "alive" }))
}
```

**Readiness** — "can it serve traffic right now?" Pings dependencies. If this fails, the
orchestrator pulls the pod _out of the load balancer_ (but doesn't restart it), so traffic
routes to healthy pods until the dependency recovers. Return 503 when not ready.

```rust
pub async fn readiness(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let start = Instant::now();
    let s = state.clone();
    let ping = tokio::task::spawn_blocking(move || s.ping())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match ping {
        Ok(()) => {
            let pool = state.pool_state();
            Ok(Json(json!({
                "status": "ready",
                "checks": {
                    "database": { "status": "up", "latency_ms": start.elapsed().as_millis() as u64 },
                    "pool": {
                        "active": pool.connections - pool.idle_connections,
                        "idle": pool.idle_connections,
                        "total": pool.connections,
                    }
                }
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, "Readiness check failed");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}
```

Both probes are **infra routes**: merged without rate-limit or timeout (see
`router-and-state.md`), so they keep answering even when the app is saturated.
