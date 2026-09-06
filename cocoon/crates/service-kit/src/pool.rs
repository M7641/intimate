use std::time::Duration;

use database::{DBActions, DatabaseConfig, DatabaseError};

// -- Pool manager --

/// r2d2 connection manager that creates database connections from a shared config.
///
/// Each pool slot holds an independent connection. The pool handles
/// concurrency, health-checks, and eviction.
pub(crate) struct DatabaseManager {
    warehouse_type: String,
}

impl r2d2::ManageConnection for DatabaseManager {
    type Connection = DBActions;
    type Error = DatabaseError;

    fn connect(&self) -> Result<DBActions, DatabaseError> {
        // Re-resolve config on every new connection so credentials are refreshed
        // (e.g. a fresh Nimbus OAuth token) instead of reusing one captured once at
        // startup. Combined with `max_lifetime` recycling connections before the
        // token's TTL, this keeps the pool authenticated without manual restarts.
        //
        // Apply a server-side statement timeout to every connector (Postgres/
        // Redshift `SET statement_timeout`, Snowflake SQL API `timeout`; a no-op
        // for DuckDB/SQLite). Default 60s, configurable via `STATEMENT_TIMEOUT_SECS`
        // — so a runaway query is cut at the DB, before the HTTP timeout fires.
        let statement_timeout =
            std::time::Duration::from_secs(env_parse("STATEMENT_TIMEOUT_SECS", 60));
        let config = DatabaseConfig::from_env(&self.warehouse_type)?
            .with_statement_timeout(Some(statement_timeout));
        DBActions::from_config(config).inspect_err(|e| {
            tracing::error!(error = %e, "Database connection attempt failed");
        })
    }

    fn is_valid(&self, conn: &mut DBActions) -> Result<(), DatabaseError> {
        // Stateless HTTP backends (Snowflake) have no socket to validate between
        // checkouts, so a `SELECT 1` here would only add a pointless — and
        // billable — SQL-API round-trip before every real query. Only stateful,
        // socket-based backends (Postgres/Redshift) get the health check. The
        // backend owns this distinction (see `DBActions::needs_liveness_check`).
        if !conn.needs_liveness_check() {
            return Ok(());
        }

        // Runs a `SELECT 1`. With `test_on_check_out`, a failure here makes r2d2
        // discard the connection and open a new one — silently retrying until the
        // connection timeout. Log it so the real cause (auth rejected, warehouse
        // suspended, bad account URL, …) is visible instead of a connection loop.
        conn.ping().inspect_err(|e| {
            tracing::error!(error = %e, "Database health check (SELECT 1) failed");
        })
    }

    fn has_broken(&self, _conn: &mut DBActions) -> bool {
        false
    }
}

// -- Public API --

/// Opaque connection pool type — callers interact via `AppState`, not r2d2 directly.
pub(crate) type ConnectionPool = r2d2::Pool<DatabaseManager>;

/// Build an r2d2 connection pool for the given warehouse backend.
///
/// Pool parameters are read from environment variables:
/// - `DB_MAX_CONNECTIONS` (default 10)
/// - `DB_CONNECTION_TIMEOUT_SECS` (default 60)
/// - `DB_IDLE_TIMEOUT_SECS` (default 600)
/// - `DB_MAX_LIFETIME_SECS` (default 1800)
pub(crate) fn build_pool(warehouse_type: &str) -> Result<ConnectionPool, DatabaseError> {
    let manager = DatabaseManager {
        warehouse_type: warehouse_type.to_string(),
    };

    let max_connections = env_parse_u32("DB_MAX_CONNECTIONS", 3);
    let connection_timeout = env_parse("DB_CONNECTION_TIMEOUT_SECS", 60);
    let idle_timeout = env_parse("DB_IDLE_TIMEOUT_SECS", 600);
    let max_lifetime = env_parse("DB_MAX_LIFETIME_SECS", 1800);

    tracing::info!(
        max_connections,
        connection_timeout_secs = connection_timeout,
        "Building database connection pool (lazy — connections open on first use)"
    );

    // `build_unchecked` creates the pool WITHOUT eagerly opening or validating
    // connections. This is deliberate: r2d2's checked `build()` blocks trying to
    // fill `min_idle`, and if the health check fails (e.g. an expired token) it
    // retries in a tight loop until the connection timeout — spamming logs and
    // wedging startup. Instead we open connections on demand, and AppState runs a
    // single connectivity probe that fails fast with the real error.
    let pool = r2d2::Pool::builder()
        .max_size(max_connections)
        // Don't pre-open or maintain idle connections; create them on demand.
        .min_idle(Some(0))
        .connection_timeout(Duration::from_secs(connection_timeout))
        .idle_timeout(Some(Duration::from_secs(idle_timeout)))
        .max_lifetime(Some(Duration::from_secs(max_lifetime)))
        .test_on_check_out(true)
        .build_unchecked(manager);

    Ok(pool)
}

/// Parse an env var as `u64`, falling back to `default`.
fn env_parse(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Parse an env var as `u32`, falling back to `default`.
fn env_parse_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}
