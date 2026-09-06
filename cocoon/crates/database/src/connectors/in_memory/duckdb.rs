pub use duckdb;

use crate::schema::{parse_table_name, schema_from_rows};
use crate::sql::{f64_to_json, substitute_params};
use crate::traits::{Database, DatabaseError, QueryResult, Row};
use serde_json::Value;

// -- Config --

/// Configuration for DuckDB connections.
#[derive(Debug, Clone)]
pub struct DuckDbConfig {
    /// Path to the database file. Use ":memory:" for in-memory database.
    pub path: String,
}

impl Default for DuckDbConfig {
    fn default() -> Self {
        Self {
            path: "duckdb.db".to_string(),
        }
    }
}

impl DuckDbConfig {
    /// Create a new DuckDB config with the given path.
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// Create an in-memory DuckDB config.
    pub fn in_memory() -> Self {
        Self {
            path: ":memory:".to_string(),
        }
    }

    /// Load configuration from environment variables.
    ///
    /// - `DUCKDB_PATH` → path (default `":memory:"`)
    pub fn from_env() -> Self {
        Self {
            path: std::env::var("DUCKDB_PATH").unwrap_or_else(|_| ":memory:".to_string()),
        }
    }
}

// -- Database implementation --

pub struct DuckDBDatabase {
    conn: duckdb::Connection,
}

impl DuckDBDatabase {
    /// Create a new connection using default configuration.
    pub fn connect() -> Result<Self, DatabaseError> {
        Self::connect_with_config(DuckDbConfig::default())
    }

    /// Create a new connection with explicit configuration.
    pub fn connect_with_config(config: DuckDbConfig) -> Result<Self, DatabaseError> {
        let conn = duckdb::Connection::open(&config.path)
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;

        Ok(Self { conn })
    }

    /// Convert a DuckDB row to our Row type (HashMap<String, Value>).
    fn convert_row(row: &duckdb::Row) -> Row {
        let mut result = Row::default();

        let column_count = row.as_ref().column_count();
        for i in 0..column_count {
            let column_name = row
                .as_ref()
                .column_name(i)
                .map(|n| n.to_string())
                .unwrap_or_else(|_| format!("column_{i}"));

            // Order matters: most specific types first, String last (it would match anything).
            let value = if let Ok(v) = row.get::<usize, bool>(i) {
                Value::Bool(v)
            } else if let Ok(v) = row.get::<usize, i64>(i) {
                Value::Number(v.into())
            } else if let Ok(v) = row.get::<usize, f64>(i) {
                f64_to_json(v)
            } else if let Ok(v) = row.get::<usize, String>(i) {
                Value::String(v)
            } else {
                Value::Null
            };

            result.insert(column_name, value);
        }

        result
    }
}

impl Database for DuckDBDatabase {
    fn query(&self, sql: &str, params: &[Value]) -> QueryResult {
        let query_str = substitute_params(sql, params);

        let mut stmt = self
            .conn
            .prepare(&query_str)
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| Ok(DuckDBDatabase::convert_row(row)))
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        Ok(rows)
    }

    fn execute(&self, sql: &str, params: &[Value]) -> Result<u64, DatabaseError> {
        let query_str = substitute_params(sql, params);

        let rows_affected = self
            .conn
            .execute(&query_str, [])
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        Ok(rows_affected as u64)
    }

    fn ping(&self) -> Result<(), DatabaseError> {
        self.conn
            .execute("SELECT 1", [])
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        Ok(())
    }

    fn close(&self) -> Result<(), DatabaseError> {
        Ok(())
    }

    fn get_table_schema(&self, table: &str) -> Result<crate::schema::TableSchema, DatabaseError> {
        let (schema_name, table_name) = parse_table_name(table);
        let schema_filter = schema_name.clone().unwrap_or_else(|| "main".to_string());
        let sql = format!(
            "SELECT column_name, data_type, ordinal_position, is_nullable \
             FROM information_schema.columns \
             WHERE table_schema = '{}' AND table_name = '{}' \
             ORDER BY ordinal_position",
            schema_filter.replace('\'', "''"),
            table_name.replace('\'', "''")
        );

        let rows = self.query(&sql, &[])?;
        schema_from_rows(rows, schema_name, table_name, "main")
    }
}
