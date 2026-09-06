pub mod copy;
pub mod insert;
pub mod merge;

pub use copy::build_copy_sql;
pub use insert::build_insert_sql;
pub use merge::{
    ColumnMapping, InsertColumns, MergeCondition, MergeConfig, UpdateColumns, build_merge_sql,
};

use crate::schema::TableSchema;
use crate::traits::{Database, DatabaseError, QueryResult};
use serde::Serialize;
use serde_json::Value;

#[cfg(feature = "duckdb")]
use crate::connectors::duckdb::{DuckDBDatabase, DuckDbConfig};

#[cfg(feature = "sqlite")]
use crate::connectors::sqlite::{SqliteConfig, SqliteDatabase};

#[cfg(feature = "postgres")]
use crate::connectors::postgres::{PostgresConfig, PostgresDatabase};

#[cfg(feature = "snowflake")]
use crate::connectors::snowflake::{SnowflakeConfig, SnowflakeDatabase};

// -- DatabaseConfig --

#[derive(Debug, Clone)]
pub enum DatabaseConfig {
    #[cfg(feature = "duckdb")]
    DuckDb(DuckDbConfig),
    #[cfg(feature = "sqlite")]
    Sqlite(SqliteConfig),
    #[cfg(feature = "postgres")]
    Postgres(PostgresConfig),
    #[cfg(feature = "snowflake")]
    Snowflake(SnowflakeConfig),
}

impl DatabaseConfig {
    /// Build the configuration for the named backend from environment variables.
    ///
    /// Accepted values:
    /// - `"duckdb"` → DuckDB (in-memory connector)
    /// - `"sqlite"` → SQLite (in-memory connector)
    /// - `"postgres"` / `"amazon_redshift"` → native Postgres (works with Redshift)
    /// - `"snowflake"` → Snowflake REST API
    pub fn from_env(backend: &str) -> Result<Self, DatabaseError> {
        match backend {
            #[cfg(feature = "duckdb")]
            "duckdb" => Ok(Self::DuckDb(DuckDbConfig::from_env())),

            #[cfg(feature = "sqlite")]
            "sqlite" => Ok(Self::Sqlite(SqliteConfig::from_env())),

            #[cfg(feature = "postgres")]
            "postgres" | "amazon_redshift" => Ok(Self::Postgres(PostgresConfig::from_env())),

            #[cfg(feature = "snowflake")]
            "snowflake" => Ok(Self::Snowflake(SnowflakeConfig::resolve()?)),

            other => Err(DatabaseError::Other(format!(
                "Unknown or disabled database backend: '{other}'. \
                 Expected: duckdb, sqlite, postgres, amazon_redshift, or snowflake \
                 (ensure the corresponding feature is enabled)"
            ))),
        }
    }

    /// Set the server-side statement timeout on backends that support one
    /// (Postgres/Redshift, Snowflake). A no-op for the in-process backends
    /// (DuckDB, SQLite), which have no remote statement to abort.
    ///
    /// `None` leaves each backend at its own default (no timeout for Postgres,
    /// the legacy 60s for Snowflake).
    pub fn with_statement_timeout(mut self, timeout: Option<std::time::Duration>) -> Self {
        match &mut self {
            #[cfg(feature = "postgres")]
            DatabaseConfig::Postgres(cfg) => cfg.statement_timeout = timeout,
            #[cfg(feature = "snowflake")]
            DatabaseConfig::Snowflake(cfg) => cfg.statement_timeout = timeout,
            #[allow(unreachable_patterns)]
            _ => {}
        }
        self
    }

    /// The connector kind this config selects, without opening a connection.
    pub fn db_type(&self) -> DBType {
        match self {
            #[cfg(feature = "duckdb")]
            DatabaseConfig::DuckDb(_) => DBType::DuckDb,
            #[cfg(feature = "sqlite")]
            DatabaseConfig::Sqlite(_) => DBType::Sqlite,
            #[cfg(feature = "postgres")]
            DatabaseConfig::Postgres(_) => DBType::Postgres,
            #[cfg(feature = "snowflake")]
            DatabaseConfig::Snowflake(_) => DBType::Snowflake,
        }
    }
}

// -- DBActions --

#[derive(Debug, Clone, Copy)]
pub enum DBType {
    #[cfg(feature = "duckdb")]
    DuckDb,
    #[cfg(feature = "sqlite")]
    Sqlite,
    #[cfg(feature = "postgres")]
    Postgres,
    #[cfg(feature = "snowflake")]
    Snowflake,
}

impl DBType {
    /// Human-readable label naming the active connector — useful for logs and
    /// status endpoints, so it is unambiguous which driver serves a request.
    pub fn label(&self) -> &'static str {
        match self {
            #[cfg(feature = "duckdb")]
            DBType::DuckDb => "DuckDB (in-process)",
            #[cfg(feature = "sqlite")]
            DBType::Sqlite => "SQLite (in-process)",
            #[cfg(feature = "postgres")]
            DBType::Postgres => "Postgres (native driver)",
            #[cfg(feature = "snowflake")]
            DBType::Snowflake => "Snowflake (REST API)",
        }
    }
}

impl std::fmt::Display for DBType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A single, synchronous database connection with a consistent API across
/// backends (dynamic dispatch via `Box<dyn Database>`).
///
/// This is the connection for **sequential, single-threaded** work — scripts,
/// workflows, one-shot jobs — where one connection used in order is exactly
/// what you want. For a concurrent API server that needs many connections at
/// once, wrap this in an [`crate::pool::ApiDbActions`] pool instead.
pub struct DBActions {
    db_type: DBType,
    db: Box<dyn Database>,
}

impl DBActions {
    /// Connect to the named database backend, reading connection parameters
    /// from environment variables.
    pub fn connect(backend: &str) -> Result<Self, DatabaseError> {
        Self::from_config(DatabaseConfig::from_env(backend)?)
    }

    /// Initialise a connection using the name of the database backend.
    ///
    /// Accepted names: `"duckdb"`, `"sqlite"`, `"postgres"`, `"amazon_redshift"`, `"snowflake"`.
    /// Connection parameters are read from environment variables.
    pub fn from_name(name: &str) -> Result<Self, DatabaseError> {
        Self::connect(name)
    }

    /// Create a new database connection using explicit configuration.
    pub fn from_config(config: DatabaseConfig) -> Result<Self, DatabaseError> {
        match config {
            #[cfg(feature = "duckdb")]
            DatabaseConfig::DuckDb(cfg) => {
                let db = DuckDBDatabase::connect_with_config(cfg)?;
                Ok(Self {
                    db_type: DBType::DuckDb,
                    db: Box::new(db),
                })
            }
            #[cfg(feature = "sqlite")]
            DatabaseConfig::Sqlite(cfg) => {
                let db = SqliteDatabase::connect_with_config(cfg)?;
                Ok(Self {
                    db_type: DBType::Sqlite,
                    db: Box::new(db),
                })
            }
            #[cfg(feature = "postgres")]
            DatabaseConfig::Postgres(cfg) => {
                let db = PostgresDatabase::connect_with_config(cfg)?;
                Ok(Self {
                    db_type: DBType::Postgres,
                    db: Box::new(db),
                })
            }
            #[cfg(feature = "snowflake")]
            DatabaseConfig::Snowflake(cfg) => {
                let db = SnowflakeDatabase::connect_with_config(cfg)?;
                Ok(Self {
                    db_type: DBType::Snowflake,
                    db: Box::new(db),
                })
            }
        }
    }

    /// Execute a query that returns rows.
    pub fn query(&self, sql: &str, params: &[Value]) -> QueryResult {
        self.db.query(sql, params)
    }

    /// Execute a statement that doesn't return rows (INSERT, UPDATE, DELETE).
    pub fn execute(&self, sql: &str, params: &[Value]) -> Result<u64, DatabaseError> {
        self.db.execute(sql, params)
    }

    /// Check if the database connection is healthy.
    pub fn ping(&self) -> Result<(), DatabaseError> {
        self.db.ping()
    }

    /// Close the database connection gracefully.
    pub fn close(&self) -> Result<(), DatabaseError> {
        self.db.close()
    }

    /// Insert structs into a table.
    ///
    /// Inserts are batched (1000 rows/batch) and wrapped in a transaction.
    pub fn insert<T: Serialize>(&self, table: &str, data: &[T]) -> Result<u64, DatabaseError> {
        let statements = build_insert_sql(table, data)?;

        if statements.is_empty() {
            return Ok(0);
        }

        self.db.execute("BEGIN", &[])?;

        let mut total_rows = 0u64;
        for sql in &statements {
            match self.db.execute(sql, &[]) {
                Ok(rows) => total_rows += rows,
                Err(e) => {
                    let _ = self.db.execute("ROLLBACK", &[]);
                    return Err(e);
                }
            }
        }

        self.db.execute("COMMIT", &[])?;
        Ok(total_rows)
    }

    /// Execute a MERGE (upsert) operation.
    pub fn merge(&self, config: &MergeConfig) -> Result<u64, DatabaseError> {
        merge::merge(self.db.as_ref(), config)
    }

    /// Get a reference to the underlying Database trait object.
    pub fn as_database(&self) -> &dyn Database {
        self.db.as_ref()
    }

    pub fn db_type(&self) -> &DBType {
        &self.db_type
    }

    /// Whether a pooled connection of this backend needs a liveness check
    /// (`SELECT 1`) before it is handed out for reuse.
    ///
    /// Stateful, socket-based backends (Postgres/Redshift) do: a pooled TCP
    /// connection can be dropped by the server or a firewall between checkouts.
    /// The Snowflake connector does not — it is stateless HTTP (a reqwest client
    /// plus an OAuth token), every query is an independent authenticated request
    /// that re-auths on 401 by itself, so there is no socket to validate and the
    /// check would only add a billable SQL-API round-trip per query.
    pub fn needs_liveness_check(&self) -> bool {
        match self.db_type {
            #[cfg(feature = "snowflake")]
            DBType::Snowflake => false,
            #[allow(unreachable_patterns)]
            _ => true,
        }
    }
}

impl Database for DBActions {
    fn query(&self, sql: &str, params: &[Value]) -> QueryResult {
        self.db.query(sql, params)
    }

    fn execute(&self, sql: &str, params: &[Value]) -> Result<u64, DatabaseError> {
        self.db.execute(sql, params)
    }

    fn ping(&self) -> Result<(), DatabaseError> {
        self.db.ping()
    }

    fn close(&self) -> Result<(), DatabaseError> {
        self.db.close()
    }

    fn get_table_schema(&self, table: &str) -> Result<TableSchema, DatabaseError> {
        self.db.get_table_schema(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[cfg(feature = "postgres")]
    #[test]
    fn with_statement_timeout_sets_postgres() {
        let cfg = DatabaseConfig::from_env("postgres")
            .unwrap()
            .with_statement_timeout(Some(Duration::from_secs(30)));
        match cfg {
            DatabaseConfig::Postgres(c) => {
                assert_eq!(c.statement_timeout, Some(Duration::from_secs(30)));
            }
            _ => panic!("expected a Postgres config"),
        }
    }

    #[cfg(feature = "duckdb")]
    #[test]
    fn with_statement_timeout_is_a_noop_for_duckdb() {
        // The in-process backend has no remote statement to abort; this must not
        // panic and must leave the variant intact.
        let cfg = DatabaseConfig::from_env("duckdb")
            .unwrap()
            .with_statement_timeout(Some(Duration::from_secs(30)));
        assert!(matches!(cfg, DatabaseConfig::DuckDb(_)));
    }
}
