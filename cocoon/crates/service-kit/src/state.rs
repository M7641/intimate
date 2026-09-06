use std::sync::Arc;
use std::time::Duration;

use database::{DBActions, DBType, DatabaseConfig, DatabaseError, Row};
use metrics_exporter_prometheus::PrometheusHandle;
use moka::sync::Cache;

use crate::circuit_breaker::CircuitBreaker;
use crate::error::AppError;
use crate::pool;
use crate::rate_limiter::RateLimiter;

/// Queries exceeding this threshold are logged as warnings and counted.
///
/// Tuned for an analytical warehouse workload, not a transactional one: multi-
/// second scans over query history are normal here, so the bar sits well above
/// a transactional app's sub-second expectation to avoid crying wolf.
const SLOW_QUERY_THRESHOLD_MS: u64 = 3000;

/// Cache of query results keyed by the exact SQL string. Values are `Arc`-wrapped
/// so a cache hit clones only a pointer, not the whole row set.
type QueryCache = Cache<String, Arc<Vec<Row>>>;

/// Build the query-result cache from the environment.
///
/// `QUERY_CACHE_TTL_SECS` sets the time-to-live (default 1800 = 30 min); `0`
/// disables caching. `QUERY_CACHE_MAX_MB` bounds the cache by **memory**, not
/// entry count (default 64 MB) — the right knob on a memory-constrained instance.
/// Each entry is weighed by [`estimate_row_bytes`], and moka evicts once the
/// total estimated footprint exceeds the budget. The warehouse endpoints are
/// read-only and change slowly, so a long TTL is safe and cuts warehouse load.
fn build_query_cache() -> Option<QueryCache> {
    let ttl = std::env::var("QUERY_CACHE_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1800u64);
    if ttl == 0 {
        tracing::debug!("Query cache disabled (QUERY_CACHE_TTL_SECS=0)");
        return None;
    }
    let max_mb = std::env::var("QUERY_CACHE_MAX_MB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(64u64);
    let max_bytes = max_mb.saturating_mul(1024 * 1024);
    tracing::debug!(
        ttl_secs = ttl,
        max_mb,
        "Query cache enabled (memory-bounded)"
    );
    Some(
        Cache::builder()
            .max_capacity(max_bytes)
            .weigher(|key: &String, value: &Arc<Vec<Row>>| -> u32 {
                let bytes = key.len().saturating_add(estimate_row_bytes(value));
                bytes.min(u32::MAX as usize) as u32
            })
            .time_to_live(Duration::from_secs(ttl))
            .build(),
    )
}

/// Approximate the heap footprint of a cached result, in bytes, for the cache's
/// memory weigher. Uses the JSON-serialized size as a cheap proxy (computed once
/// per cache miss), scaled up to account for the larger in-memory layout
/// (`HashMap` buckets + per-`String`/`Value` allocations) so the memory bound
/// stays conservative on a small instance.
fn estimate_row_bytes(rows: &[Row]) -> usize {
    serde_json::to_vec(rows)
        .map(|v| v.len().saturating_mul(2))
        .unwrap_or(0)
}

// -- App state --

/// Shared application state, held via r2d2::Pool (internally Arc-backed)
/// for cheap cloning across Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pool: pool::ConnectionPool,
    backend: String,
    connector: DBType,
    metrics_handle: PrometheusHandle,
    circuit_breaker: CircuitBreaker,
    rate_limiter: RateLimiter,
    heavy_rate_limiter: RateLimiter,
    query_cache: Option<QueryCache>,
}

impl AppState {
    /// Connect to the database backend selected at runtime via `DATA_WAREHOUSE_TYPE`.
    ///
    /// `warehouse_type` is the raw value from the environment variable:
    /// `"amazon_redshift"`, `"snowflake"`, or `"duckdb"`.
    pub fn new(
        warehouse_type: &str,
        metrics_handle: PrometheusHandle,
        rate_limiter: RateLimiter,
        heavy_rate_limiter: RateLimiter,
    ) -> Result<Self, DatabaseError> {
        // Resolve the connector once so we can log it and report it, making it
        // unambiguous which backend driver is serving requests.
        let config = DatabaseConfig::from_env(warehouse_type)?;
        let connector = config.db_type();
        tracing::info!(
            warehouse_type,
            connector = %connector,
            "Database connector selected"
        );

        // The pool re-resolves config per connection (refreshing credentials such
        // as the Nimbus OAuth token), so it takes the backend name, not a fixed config.
        let pool = pool::build_pool(warehouse_type)?;

        // One-shot connectivity probe: open a single connection and run the health
        // check exactly once. A failure surfaces its real error immediately here,
        // instead of the pool retrying a bad connection until the connection
        // timeout (which floods the logs with reconnect attempts).
        tracing::debug!("Verifying database connectivity...");
        DBActions::from_config(config)?.ping()?;
        tracing::debug!("Database connection verified");

        Ok(Self {
            pool,
            backend: warehouse_type.to_string(),
            connector,
            metrics_handle,
            circuit_breaker: CircuitBreaker::new(),
            rate_limiter,
            heavy_rate_limiter,
            query_cache: build_query_cache(),
        })
    }

    /// The raw warehouse type string (e.g. `"amazon_redshift"`, `"snowflake"`, `"duckdb"`).
    pub fn backend(&self) -> &str {
        &self.backend
    }

    /// The resolved connector kind actually serving requests.
    pub fn connector(&self) -> DBType {
        self.connector
    }

    /// Check that at least one connection is healthy.
    pub fn ping(&self) -> Result<(), DatabaseError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        conn.ping()
    }

    /// Render Prometheus metrics in exposition format.
    pub fn render_metrics(&self) -> String {
        self.metrics_handle.render()
    }

    /// Current connection pool statistics.
    pub fn pool_state(&self) -> r2d2::State {
        self.pool.state()
    }

    /// Global rate limiter.
    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }

    /// Rate limiter for heavy/expensive endpoints.
    pub fn heavy_rate_limiter(&self) -> &RateLimiter {
        &self.heavy_rate_limiter
    }

    /// Execute a blocking database query on tokio's blocking thread pool.
    ///
    /// Each call checks out a connection from the pool, runs the synchronous
    /// query, then returns the connection. Emits Prometheus metrics for
    /// query duration, success/failure counts, pool utilisation, and slow queries.
    pub async fn blocking_query(&self, sql: String) -> Result<Vec<Row>, AppError> {
        // Serve from cache when a fresh result exists. The warehouse endpoints are
        // read-only and their data changes slowly, so a cached result within the
        // TTL is authoritative — a hit skips the DB (and the circuit breaker).
        if let Some(cache) = &self.query_cache {
            if let Some(rows) = cache.get(&sql) {
                metrics::counter!("db_query_cache_total", "result" => "hit").increment(1);
                tracing::debug!(sql = %sql, "Query cache hit");
                return Ok((*rows).clone());
            }
        }

        // Circuit breaker gate — fail fast if the DB is known to be down.
        self.circuit_breaker.check()?;
        metrics::gauge!("db_circuit_breaker_state").set(self.circuit_breaker.state_gauge());

        tracing::debug!(sql = %sql, "Executing query");

        let pool = self.pool.clone();
        let sql_ref = sql.clone();
        let start = std::time::Instant::now();

        let result: Result<Vec<Row>, DatabaseError> = tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
            conn.query(&sql, &[])
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join error: {e}")))?;

        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;

        // Record pool gauge metrics on every query (cheap sampling point).
        let ps = self.pool.state();
        metrics::gauge!("db_pool_connections_total").set(f64::from(ps.connections));
        metrics::gauge!("db_pool_connections_idle").set(f64::from(ps.idle_connections));

        match result {
            Ok(rows) => {
                self.circuit_breaker.record_success();
                metrics::counter!("db_queries_total", "status" => "success").increment(1);
                metrics::histogram!("db_query_duration_seconds").record(elapsed.as_secs_f64());

                if elapsed_ms > SLOW_QUERY_THRESHOLD_MS {
                    tracing::warn!(
                        sql = %sql_ref,
                        elapsed_ms,
                        threshold_ms = SLOW_QUERY_THRESHOLD_MS,
                        "Slow query detected"
                    );
                    metrics::counter!("db_slow_queries_total").increment(1);
                }

                tracing::debug!(rows = rows.len(), elapsed_ms, "Query completed");

                // Populate the cache so the next identical query is served within
                // the TTL without touching the warehouse.
                if let Some(cache) = &self.query_cache {
                    cache.insert(sql_ref.clone(), Arc::new(rows.clone()));
                    metrics::counter!("db_query_cache_total", "result" => "miss").increment(1);
                }

                Ok(rows)
            }
            Err(err) => {
                self.circuit_breaker.record_failure();
                metrics::counter!("db_queries_total", "status" => "error").increment(1);
                metrics::histogram!("db_query_duration_seconds").record(elapsed.as_secs_f64());

                tracing::error!(
                    error = %err,
                    sql = %sql_ref,
                    elapsed_ms,
                    "Query failed"
                );
                Err(AppError::from(err))
            }
        }
    }

    /// Run a DML/DDL statement (INSERT / MERGE / CREATE TABLE …) and return the
    /// number of rows the warehouse reports affected.
    ///
    /// Unlike [`Self::blocking_query`] this is a **write** path: it deliberately
    /// bypasses the read-through query cache (caching a mutation makes no sense),
    /// but keeps the same circuit-breaker gating, pool-gauge sampling, and
    /// success/error metrics.
    pub async fn blocking_execute(&self, sql: String) -> Result<u64, AppError> {
        self.run_uncached("execute", sql, |conn, sql| conn.execute(sql, &[]))
            .await
    }

    /// Run a read query while **bypassing the result cache**, for callers that
    /// need a live answer every time — e.g. a `COUNT(*)` saturation probe or the
    /// `MAX(end_time)` watermark, where a cached value would be stale and wrong.
    pub async fn blocking_query_uncached(&self, sql: String) -> Result<Vec<Row>, AppError> {
        self.run_uncached("query", sql, |conn, sql| conn.query(sql, &[]))
            .await
    }

    /// Shared machinery for a single blocking DB call with no caching layer:
    /// circuit-breaker gate, blocking-pool checkout, timing, pool-gauge sampling,
    /// and success/error metrics. The `op` closure performs the actual work on a
    /// checked-out connection. Used by the write and uncached-read paths above.
    async fn run_uncached<T, F>(
        &self,
        kind: &'static str,
        sql: String,
        op: F,
    ) -> Result<T, AppError>
    where
        T: Send + 'static,
        F: FnOnce(&DBActions, &str) -> Result<T, DatabaseError> + Send + 'static,
    {
        self.circuit_breaker.check()?;
        metrics::gauge!("db_circuit_breaker_state").set(self.circuit_breaker.state_gauge());

        tracing::debug!(sql = %sql, kind, "Executing statement");

        let pool = self.pool.clone();
        let sql_ref = sql.clone();
        let start = std::time::Instant::now();

        let result: Result<T, DatabaseError> = tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
            op(&conn, &sql)
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join error: {e}")))?;

        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;

        let ps = self.pool.state();
        metrics::gauge!("db_pool_connections_total").set(f64::from(ps.connections));
        metrics::gauge!("db_pool_connections_idle").set(f64::from(ps.idle_connections));

        match result {
            Ok(value) => {
                self.circuit_breaker.record_success();
                metrics::counter!("db_queries_total", "status" => "success").increment(1);
                metrics::histogram!("db_query_duration_seconds").record(elapsed.as_secs_f64());
                if elapsed_ms > SLOW_QUERY_THRESHOLD_MS {
                    tracing::warn!(
                        sql = %sql_ref,
                        elapsed_ms,
                        threshold_ms = SLOW_QUERY_THRESHOLD_MS,
                        "Slow statement detected"
                    );
                    metrics::counter!("db_slow_queries_total").increment(1);
                }
                tracing::debug!(elapsed_ms, kind, "Statement completed");
                Ok(value)
            }
            Err(err) => {
                self.circuit_breaker.record_failure();
                metrics::counter!("db_queries_total", "status" => "error").increment(1);
                metrics::histogram!("db_query_duration_seconds").record(elapsed.as_secs_f64());
                tracing::error!(error = %err, sql = %sql_ref, elapsed_ms, "Statement failed");
                Err(AppError::from(err))
            }
        }
    }
}
