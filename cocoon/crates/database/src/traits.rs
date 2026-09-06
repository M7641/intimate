use std::collections::HashMap;

use crate::schema::TableSchema;

#[derive(Debug)]
pub enum DatabaseError {
    ConnectionError(String),
    QueryError(String),
    NotFound,
    Duplicate,
    SerializationError(String),
    ValidationError(String),
    Other(String),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            DatabaseError::QueryError(msg) => write!(f, "Query error: {}", msg),
            DatabaseError::NotFound => write!(f, "Record not found"),
            DatabaseError::Duplicate => write!(f, "Duplicate record"),
            DatabaseError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            DatabaseError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            DatabaseError::Other(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl std::error::Error for DatabaseError {}

#[cfg(feature = "duckdb")]
impl From<duckdb::Error> for DatabaseError {
    fn from(e: duckdb::Error) -> Self {
        DatabaseError::QueryError(e.to_string())
    }
}

#[cfg(feature = "sqlite")]
impl From<rusqlite::Error> for DatabaseError {
    fn from(e: rusqlite::Error) -> Self {
        DatabaseError::QueryError(e.to_string())
    }
}

#[cfg(feature = "postgres")]
impl From<postgres::Error> for DatabaseError {
    fn from(e: postgres::Error) -> Self {
        DatabaseError::QueryError(e.to_string())
    }
}

pub type Row = HashMap<String, serde_json::Value>;
pub type QueryResult = Result<Vec<Row>, DatabaseError>;

/// Core database trait — connect and query a SQL database.
///
/// Methods take `&self` because querying is not conceptually a mutation.
/// Connectors that need internal mutability (e.g. Postgres) handle it
/// internally via `RefCell`.
pub trait Database: Send {
    /// Execute a raw SQL query that returns rows
    fn query(&self, sql: &str, params: &[serde_json::Value]) -> QueryResult;

    /// Execute a raw SQL query that doesn't return rows
    fn execute(&self, sql: &str, params: &[serde_json::Value]) -> Result<u64, DatabaseError>;

    /// Check if the database connection is healthy
    fn ping(&self) -> Result<(), DatabaseError>;

    /// Close the database connection
    fn close(&self) -> Result<(), DatabaseError>;

    /// Get the schema for a table (column names, types, order).
    fn get_table_schema(&self, table: &str) -> Result<TableSchema, DatabaseError>;
}
