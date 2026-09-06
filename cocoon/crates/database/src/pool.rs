use r2d2::{ManageConnection, Pool, PooledConnection};
use serde_json::Value;

use crate::actions::{DBActions, DBType, DatabaseConfig};
use crate::schema::TableSchema;
use crate::traits::{Database, DatabaseError, QueryResult};

/// R2D2 connection manager — creates `DBActions` instances from a `DatabaseConfig`.
pub struct ApiDbActionsManager {
    config: DatabaseConfig,
}

impl ManageConnection for ApiDbActionsManager {
    type Connection = DBActions;
    type Error = DatabaseError;

    fn connect(&self) -> Result<DBActions, DatabaseError> {
        DBActions::from_config(self.config.clone())
    }

    fn is_valid(&self, conn: &mut DBActions) -> Result<(), DatabaseError> {
        conn.ping()
    }

    fn has_broken(&self, _conn: &mut DBActions) -> bool {
        false
    }
}

/// Pooled, concurrency-safe database access for **API server** use.
///
/// Wraps an r2d2 pool of [`DBActions`] connections so many in-flight requests
/// can each check out their own connection. `Clone` (internal `Arc`),
/// `Send + Sync`; each pool slot is one independent backend connection.
///
/// For sequential, single-connection work (scripts, workflows, one-shot jobs)
/// use a plain [`DBActions`] instead — same API, without the pool.
#[derive(Clone)]
pub struct ApiDbActions {
    pool: Pool<ApiDbActionsManager>,
    db_type: DBType,
}

impl ApiDbActions {
    /// Build a pool from a backend name (e.g. `"duckdb"`, `"postgres"`),
    /// reading connection parameters from environment variables.
    ///
    /// `max_size` lets the caller flex the pool to the deployment (e.g. read it
    /// from an env var at startup). `None` uses the default of 4.
    pub fn connect(backend: &str, max_size: Option<u32>) -> Result<Self, DatabaseError> {
        Self::connect_with_timeout(backend, max_size, None)
    }

    /// Like [`connect`](Self::connect), but also applies a server-side statement
    /// timeout (see [`DatabaseConfig::with_statement_timeout`]). Every pooled
    /// connection inherits it, so a runaway statement is aborted by the warehouse
    /// and its connection returned to the pool.
    pub fn connect_with_timeout(
        backend: &str,
        max_size: Option<u32>,
        statement_timeout: Option<std::time::Duration>,
    ) -> Result<Self, DatabaseError> {
        let config = DatabaseConfig::from_env(backend)?.with_statement_timeout(statement_timeout);
        Self::new(config, max_size)
    }

    /// Build a pool from the given config with an optional max size (default: 4).
    pub fn new(config: DatabaseConfig, max_size: Option<u32>) -> Result<Self, DatabaseError> {
        let db_type = config.db_type();
        let manager = ApiDbActionsManager { config };
        let pool = Pool::builder()
            .max_size(max_size.unwrap_or(4))
            .build(manager)
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        Ok(Self { pool, db_type })
    }

    /// Checkout a connection from the pool (blocks if all are busy).
    pub fn get(&self) -> Result<PooledConnection<ApiDbActionsManager>, DatabaseError> {
        self.pool
            .get()
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))
    }

    pub fn db_type(&self) -> &DBType {
        &self.db_type
    }

    // -- Convenience methods: borrow a connection and delegate --

    pub fn query(&self, sql: &str, params: &[Value]) -> QueryResult {
        self.get()?.query(sql, params)
    }

    pub fn execute(&self, sql: &str, params: &[Value]) -> Result<u64, DatabaseError> {
        self.get()?.execute(sql, params)
    }

    pub fn ping(&self) -> Result<(), DatabaseError> {
        self.get()?.ping()
    }

    pub fn get_table_schema(&self, table: &str) -> Result<TableSchema, DatabaseError> {
        self.get()?.get_table_schema(table)
    }
}
