use std::cell::RefCell;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use postgres::types::Type;
use rust_decimal::Decimal;
use serde_json::Value;

use crate::schema::{parse_table_name, schema_from_rows};
use crate::sql::{f64_to_json, parse_text_value, substitute_params};
use crate::traits::{Database, DatabaseError, QueryResult, Row};

// -- Config --

/// Configuration for native Postgres/Redshift connections.
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    /// TLS mode: "require", "prefer", or "disable".
    pub ssl_mode: String,
    /// Accept invalid (self-signed) TLS certificates.
    pub accept_invalid_certs: bool,
    /// Server-side `statement_timeout` applied to every statement on the
    /// connection. `None` (the default) leaves it unset — no timeout, suitable
    /// for long analytical queries. Set it for ingest-style workloads where a
    /// runaway statement should be aborted server-side. See
    /// [`DatabaseConfig::with_statement_timeout`](crate::DatabaseConfig::with_statement_timeout).
    pub statement_timeout: Option<Duration>,
}

impl PostgresConfig {
    /// Build configuration from environment variables.
    ///
    /// Uses the same env vars as the old Postgres/Redshift connector:
    /// `REDSHIFT_HOST`, `REDSHIFT_PORT`, `REDSHIFT_DATABASE`,
    /// `REDSHIFT_USERNAME`, `REDSHIFT_PASSWORD`, `REDSHIFT_SSL_MODE`.
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("REDSHIFT_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("REDSHIFT_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(5439),
            database: std::env::var("REDSHIFT_DATABASE").unwrap_or_else(|_| "postgres".to_string()),
            username: std::env::var("REDSHIFT_USERNAME").unwrap_or_else(|_| "postgres".to_string()),
            password: std::env::var("REDSHIFT_PASSWORD").unwrap_or_default(),
            ssl_mode: std::env::var("REDSHIFT_SSL_MODE").unwrap_or_else(|_| "require".to_string()),
            accept_invalid_certs: std::env::var("REDSHIFT_ACCEPT_INVALID_CERTS")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
            // Off by default; callers opt in via `with_statement_timeout`.
            statement_timeout: None,
        }
    }
}

// -- Database implementation --

/// `postgres::Client` requires `&mut self` for queries, but that is an
/// implementation detail of the wire protocol, not a semantic property of
/// "querying a database". We use `RefCell` for interior mutability so
/// the `Database` trait can stay `&self`.
pub struct PostgresDatabase {
    /// `Option` so [`Drop`] can `take()` the client and close it elsewhere.
    /// Always `Some` for the connection's whole usable life.
    client: RefCell<Option<postgres::Client>>,
    default_schema: String,
}

impl PostgresDatabase {
    /// Create a new connection from environment variables.
    pub fn connect() -> Result<Self, DatabaseError> {
        Self::connect_with_config(PostgresConfig::from_env())
    }

    /// Create a new connection with explicit configuration.
    pub fn connect_with_config(config: PostgresConfig) -> Result<Self, DatabaseError> {
        let client = if config.ssl_mode == "disable" {
            let conn_str = format!(
                "host={} port={} dbname={} user={} password={}",
                config.host, config.port, config.database, config.username, config.password
            );
            postgres::Client::connect(&conn_str, postgres::NoTls)
                .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?
        } else {
            let mut tls_builder = native_tls::TlsConnector::builder();
            if config.accept_invalid_certs {
                tls_builder.danger_accept_invalid_certs(true);
                tls_builder.danger_accept_invalid_hostnames(true);
            }
            let tls_connector = tls_builder
                .build()
                .map_err(|e| DatabaseError::ConnectionError(format!("TLS error: {e}")))?;

            let pg_tls = postgres_native_tls::MakeTlsConnector::new(tls_connector);

            let conn_str = format!(
                "host={} port={} dbname={} user={} password={} sslmode={}",
                config.host,
                config.port,
                config.database,
                config.username,
                config.password,
                config.ssl_mode
            );
            postgres::Client::connect(&conn_str, pg_tls)
                .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?
        };

        let mut client = client;
        // Apply the server-side statement timeout once for this connection, so
        // the warehouse itself aborts a runaway statement and frees the socket.
        // Set on the connection (not per query) so it covers every statement,
        // including the long-running COPY. `statement_timeout` is supported by
        // both modern Postgres and Redshift (despite Redshift's older lineage).
        if let Some(timeout) = config.statement_timeout {
            let ms = timeout.as_millis();
            client
                .batch_execute(&format!("SET statement_timeout = {ms}"))
                .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        }

        Ok(Self {
            client: RefCell::new(Some(client)),
            default_schema: "public".to_string(),
        })
    }

    /// Convert a Postgres row to our Row type (HashMap<String, Value>).
    fn convert_row(row: &postgres::Row) -> Row {
        let mut result = Row::default();

        for (i, column) in row.columns().iter().enumerate() {
            let name = column.name().to_string();
            let value = convert_column_value(row, i, column.type_());
            result.insert(name, value);
        }

        result
    }
}

/// Convert a single column value from a Postgres row to a serde_json::Value.
///
/// Uses the column's `Type` metadata to pick the right getter.
/// Falls back to text representation for unknown types.
pub(crate) fn convert_column_value(row: &postgres::Row, idx: usize, col_type: &Type) -> Value {
    // Try NULL first — any type can be NULL.
    // We check by attempting a common type; if the value is NULL,
    // get::<_, Option<T>> returns Ok(None).

    match *col_type {
        Type::BOOL => row
            .get::<_, Option<bool>>(idx)
            .map_or(Value::Null, Value::Bool),

        Type::INT2 => row
            .get::<_, Option<i16>>(idx)
            .map_or(Value::Null, |v| Value::Number(i64::from(v).into())),

        Type::INT4 => row
            .get::<_, Option<i32>>(idx)
            .map_or(Value::Null, |v| Value::Number(i64::from(v).into())),

        Type::INT8 => row
            .get::<_, Option<i64>>(idx)
            .map_or(Value::Null, |v| Value::Number(v.into())),

        Type::FLOAT4 => row
            .get::<_, Option<f32>>(idx)
            .map_or(Value::Null, |v| f64_to_json(f64::from(v))),

        Type::FLOAT8 => row
            .get::<_, Option<f64>>(idx)
            .map_or(Value::Null, f64_to_json),

        Type::NUMERIC => row
            .get::<_, Option<Decimal>>(idx)
            .map_or(Value::Null, |d| parse_text_value(&d.to_string())),

        Type::VARCHAR | Type::TEXT | Type::BPCHAR | Type::NAME => row
            .get::<_, Option<String>>(idx)
            .map_or(Value::Null, Value::String),

        Type::JSON | Type::JSONB => row
            .get::<_, Option<serde_json::Value>>(idx)
            .map_or(Value::Null, |v| v),

        Type::TIMESTAMP => row
            .get::<_, Option<NaiveDateTime>>(idx)
            .map_or(Value::Null, |v| Value::String(v.to_string())),

        Type::TIMESTAMPTZ => row
            .get::<_, Option<DateTime<Utc>>>(idx)
            .map_or(Value::Null, |v| Value::String(v.to_rfc3339())),

        Type::DATE => row
            .get::<_, Option<NaiveDate>>(idx)
            .map_or(Value::Null, |v| Value::String(v.to_string())),

        Type::TIME | Type::TIMETZ => row
            .get::<_, Option<NaiveTime>>(idx)
            .map_or(Value::Null, |v| Value::String(v.to_string())),

        // Fallback: cascade through common types using try_get (non-panicking).
        _ => {
            if let Ok(Some(s)) = row.try_get::<_, Option<String>>(idx) {
                return parse_text_value(&s);
            }
            if let Ok(Some(v)) = row.try_get::<_, Option<i64>>(idx) {
                return Value::Number(v.into());
            }
            if let Ok(Some(v)) = row.try_get::<_, Option<f64>>(idx) {
                return f64_to_json(v);
            }
            if let Ok(Some(v)) = row.try_get::<_, Option<bool>>(idx) {
                return Value::Bool(v);
            }
            Value::Null
        }
    }
}

impl Drop for PostgresDatabase {
    /// Close the connection on a thread with no tokio runtime in context.
    ///
    /// The sync `postgres` client's own `Drop` runs a blocking `block_on` to send
    /// the TCP terminate message. If that fires on a thread already driving our
    /// tokio runtime — the main `block_on` thread, or a worker — tokio panics
    /// with "cannot start a runtime from within a runtime". This happens for ANY
    /// drop site (pool teardown, r2d2 reaping, startup), not just clean shutdown,
    /// so we localise the fix here rather than guarding each caller.
    ///
    /// A freshly spawned std thread is guaranteed runtime-free; `join` waits for
    /// the socket to close so the connection isn't severed mid-flush. These drops
    /// are rare (pooled, long-lived connections), so the per-close thread is cheap.
    fn drop(&mut self) {
        if let Some(client) = self.client.borrow_mut().take() {
            let _ = std::thread::spawn(move || drop(client)).join();
        }
    }
}

impl Database for PostgresDatabase {
    fn query(&self, sql: &str, params: &[Value]) -> QueryResult {
        let query_str = substitute_params(sql, params);
        let mut guard = self.client.borrow_mut();
        let client = guard.as_mut().expect("postgres client present");

        let rows = client
            .query(&*query_str, &[])
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        Ok(rows.iter().map(Self::convert_row).collect())
    }

    fn execute(&self, sql: &str, params: &[Value]) -> Result<u64, DatabaseError> {
        let query_str = substitute_params(sql, params);
        let mut guard = self.client.borrow_mut();
        let client = guard.as_mut().expect("postgres client present");

        let rows_affected = client
            .execute(&*query_str, &[])
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        Ok(rows_affected)
    }

    fn ping(&self) -> Result<(), DatabaseError> {
        self.query("SELECT 1", &[])?;
        Ok(())
    }

    fn close(&self) -> Result<(), DatabaseError> {
        Ok(())
    }

    fn get_table_schema(&self, table: &str) -> Result<crate::schema::TableSchema, DatabaseError> {
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

        let rows = self.query(&sql, &[])?;
        schema_from_rows(rows, schema_name, table_name, &self.default_schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postgres_config_from_env() {
        unsafe {
            std::env::set_var("REDSHIFT_HOST", "myhost");
            std::env::set_var("REDSHIFT_PORT", "5439");
            std::env::set_var("REDSHIFT_DATABASE", "mydb");
            std::env::set_var("REDSHIFT_USERNAME", "user");
            std::env::set_var("REDSHIFT_PASSWORD", "pass");
        }

        let config = PostgresConfig::from_env();

        assert_eq!(config.host, "myhost");
        assert_eq!(config.port, 5439);
        assert_eq!(config.database, "mydb");
        assert_eq!(config.username, "user");
        assert_eq!(config.password, "pass");
        assert_eq!(config.ssl_mode, "require");
    }
}
