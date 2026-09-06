//! Spike: async Postgres/Redshift connector on `tokio-postgres` + `deadpool`.
//!
//! Parallel to the synchronous [`super::postgres::PostgresDatabase`]. It exists
//! to evaluate moving off the blocking `postgres` crate, whose hidden current-
//! thread runtime panics ("cannot start a runtime from within a runtime") when
//! its internal `block_on` runs on a thread that is already driving our tokio
//! runtime.
//!
//! The point of staying inside the `rust-postgres` family: `tokio_postgres::Row`
//! and `tokio_postgres::types::Type` ARE the types the sync `postgres` crate
//! re-exports, so the row -> JSON conversion is reused verbatim via
//! [`super::postgres::convert_column_value`] — zero new mapping code.
//!
//! This is deliberately NOT an `impl Database`: that trait is synchronous, and
//! making it async is the ripple we want to measure separately. The API here is
//! inherent `async fn`s, enough to exercise the driver, the pool, and TLS.

use deadpool_postgres::{Config as PoolConfig, ManagerConfig, Pool, RecyclingMethod, Runtime};
use serde_json::Value;

use super::postgres::{PostgresConfig, convert_column_value};
use crate::schema::{TableSchema, parse_table_name, schema_from_rows};
use crate::sql::substitute_params;
use crate::traits::{DatabaseError, QueryResult, Row};

/// Async Postgres/Redshift backend over a `deadpool` connection pool.
///
/// `deadpool_postgres::Pool` is a concrete type (the TLS connector is erased
/// inside its manager), so the `NoTls` and native-TLS branches both produce the
/// same `Pool` — no generic plumbing leaks into this struct.
pub struct AsyncPostgresDatabase {
    pool: Pool,
    default_schema: String,
}

impl AsyncPostgresDatabase {
    /// Build a pool from environment variables (same `REDSHIFT_*` vars as the
    /// sync connector). Pool construction is synchronous; connections are opened
    /// lazily on first checkout — no boot-time `spawn_blocking` dance needed.
    pub fn connect() -> Result<Self, DatabaseError> {
        Self::with_config(PostgresConfig::from_env())
    }

    /// Build a pool from explicit configuration.
    pub fn with_config(config: PostgresConfig) -> Result<Self, DatabaseError> {
        let mut cfg = PoolConfig::new();
        cfg.host = Some(config.host.clone());
        cfg.port = Some(config.port);
        cfg.dbname = Some(config.database.clone());
        cfg.user = Some(config.username.clone());
        cfg.password = Some(config.password.clone());
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });

        let pool = if config.ssl_mode == "disable" {
            cfg.create_pool(Some(Runtime::Tokio1), tokio_postgres::NoTls)
                .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?
        } else {
            // Reuse the repo's existing native-tls path for the spike. Swapping
            // this for `tokio-postgres-rustls` later is a drop-in MakeTlsConnect
            // change and removes the OpenSSL/native-tls system dependency.
            let mut tls_builder = native_tls::TlsConnector::builder();
            if config.accept_invalid_certs {
                tls_builder.danger_accept_invalid_certs(true);
                tls_builder.danger_accept_invalid_hostnames(true);
            }
            let connector = tls_builder
                .build()
                .map_err(|e| DatabaseError::ConnectionError(format!("TLS error: {e}")))?;
            let tls = postgres_native_tls::MakeTlsConnector::new(connector);
            cfg.create_pool(Some(Runtime::Tokio1), tls)
                .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?
        };

        Ok(Self {
            pool,
            default_schema: "public".to_string(),
        })
    }

    /// Execute a raw SQL query that returns rows.
    pub async fn query(&self, sql: &str, params: &[Value]) -> QueryResult {
        let query_str = substitute_params(sql, params);
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        let rows = client
            .query(query_str.as_str(), &[])
            .await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;
        Ok(rows.iter().map(row_to_json).collect())
    }

    /// Execute a raw SQL statement that doesn't return rows.
    pub async fn execute(&self, sql: &str, params: &[Value]) -> Result<u64, DatabaseError> {
        let query_str = substitute_params(sql, params);
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        client
            .execute(query_str.as_str(), &[])
            .await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))
    }

    /// Check the connection is healthy.
    pub async fn ping(&self) -> Result<(), DatabaseError> {
        self.query("SELECT 1", &[]).await?;
        Ok(())
    }

    /// Get the schema for a table (column names, types, order).
    pub async fn get_table_schema(&self, table: &str) -> Result<TableSchema, DatabaseError> {
        let (schema_name, table_name) = parse_table_name(table);
        let schema_filter = schema_name
            .clone()
            .unwrap_or_else(|| self.default_schema.clone());

        let sql = format!(
            "SELECT column_name, data_type, ordinal_position, is_nullable \
             FROM information_schema.columns \
             WHERE table_schema = '{}' AND table_name = '{}' \
             ORDER BY ordinal_position",
            schema_filter.replace('\'', "''"),
            table_name.replace('\'', "''")
        );

        let rows = self.query(&sql, &[]).await?;
        schema_from_rows(rows, schema_name, table_name, &self.default_schema)
    }
}

/// Convert a `tokio_postgres::Row` to our `Row` (HashMap<String, Value>).
///
/// Delegates to the sync connector's `convert_column_value`: the row and type
/// values here are the same `postgres-types` instances that function already
/// handles, so the entire type-mapping table is shared, not duplicated.
fn row_to_json(row: &tokio_postgres::Row) -> Row {
    let mut out = Row::default();
    for (i, column) in row.columns().iter().enumerate() {
        out.insert(
            column.name().to_string(),
            convert_column_value(row, i, column.type_()),
        );
    }
    out
}
