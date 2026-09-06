# Timeouts

> Status: **Implemented**

## What

A maximum duration for any single request. If a handler or downstream call exceeds the timeout, the request is cancelled and the client receives a 408 or 504 response.

## Why

Without timeouts, a single slow query or a hung downstream service consumes resources indefinitely:

| Scenario                              | Without timeout                                                                  | With timeout                                                             |
| ------------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| DB query hangs due to lock contention | Tokio task holds a pool connection forever → pool exhaustion → cascading failure | Query cancelled after 30s → connection returned → other requests succeed |
| Client disconnects mid-request        | Server keeps processing, wasting CPU and DB time                                 | Server detects cancellation, drops the work                              |
| Slow endpoint under load              | Thread pool saturated → all endpoints degrade                                    | Slow requests are shed → healthy endpoints remain fast                   |

Timeouts are a **safety net**. They set an upper bound on how badly a single request can impact the system.

## How — Implementation

### Architecture

Two timeout tiers are applied via custom `axum::middleware::from_fn` middleware using `tokio::time::timeout()`:

1. **Global timeout** — applied to all routes
2. **Heavy route timeout** — applied to expensive data-scanning endpoints

### Global timeout

All requests are wrapped with a configurable deadline. If the handler does not complete in time, the middleware returns a `504 Gateway Timeout` with a JSON body:

```json
{ "detail": "Request timed out" }
```

- **Default:** 30 seconds
- **Env var:** `GLOBAL_TIMEOUT_SECS`
- **File:** `src/middleware.rs` (`global_timeout`)

### Heavy route timeout

Endpoints that perform full table scans or column-level aggregations have a separate, longer timeout:

- **Default:** 120 seconds
- **Env var:** `HEAVY_TIMEOUT_SECS`
- **File:** `src/middleware.rs` (`heavy_route_timeout`)

Heavy routes are split into a dedicated router in `src/routes/data_view/mod.rs` so the middleware applies only to them:

| Heavy route                                      | Purpose                   |
| ------------------------------------------------ | ------------------------- |
| `/data/{table_name}`                             | Full table data fetch     |
| `/column_values/{table_name}/{column_name}`      | Distinct column values    |
| `/value_distribution/{table_name}/{column_name}` | Column value distribution |

### Query-level timeout

The database connection pool also enforces its own timeout:

```rust
.connection_timeout(Duration::from_secs(30))  // set in pool config
```

### Metrics

- `http_timeouts_total` — counter incremented each time a request is terminated by the timeout middleware
