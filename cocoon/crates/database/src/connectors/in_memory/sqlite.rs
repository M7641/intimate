use crate::schema::{ColumnInfo, TableSchema, parse_table_name};
use crate::sql::{f64_to_json, substitute_params};
use crate::traits::{Database, DatabaseError, QueryResult, Row};
use serde_json::Value;

// -- Config --

/// Configuration for SQLite connections.
#[derive(Debug, Clone)]
pub struct SqliteConfig {
    /// Path to the database file. Use ":memory:" for in-memory database.
    pub path: String,
}

impl Default for SqliteConfig {
    fn default() -> Self {
        Self {
            path: ":memory:".to_string(),
        }
    }
}

impl SqliteConfig {
    /// Create a new SQLite config with the given path.
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// Create an in-memory SQLite config.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Load configuration from environment variables.
    ///
    /// - `SQLITE_PATH` → path (default `":memory:"`)
    pub fn from_env() -> Self {
        Self {
            path: std::env::var("SQLITE_PATH").unwrap_or_else(|_| ":memory:".to_string()),
        }
    }
}

// -- Database implementation --

pub struct SqliteDatabase {
    conn: rusqlite::Connection,
}

impl SqliteDatabase {
    /// Create a new connection using default configuration (in-memory).
    pub fn connect() -> Result<Self, DatabaseError> {
        Self::connect_with_config(SqliteConfig::default())
    }

    /// Create a new connection with explicit configuration.
    pub fn connect_with_config(config: SqliteConfig) -> Result<Self, DatabaseError> {
        let conn = if config.path == ":memory:" {
            rusqlite::Connection::open_in_memory()
        } else {
            rusqlite::Connection::open(&config.path)
        }
        .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;

        Ok(Self { conn })
    }
}

impl Database for SqliteDatabase {
    fn query(&self, sql: &str, params: &[Value]) -> QueryResult {
        let query_str = substitute_params(sql, params);

        let mut stmt = self
            .conn
            .prepare(&query_str)
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let column_names: Vec<String> = (0..stmt.column_count())
            .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
            .collect();

        let mut result = Vec::new();
        let mut rows = stmt
            .query([])
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        while let Some(row) = rows
            .next()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?
        {
            let mut map = Row::default();
            for (i, name) in column_names.iter().enumerate() {
                let value = match row.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Null) => Value::Null,
                    Ok(rusqlite::types::ValueRef::Integer(n)) => Value::Number(n.into()),
                    Ok(rusqlite::types::ValueRef::Real(f)) => f64_to_json(f),
                    Ok(rusqlite::types::ValueRef::Text(bytes)) => {
                        Value::String(String::from_utf8_lossy(bytes).into_owned())
                    }
                    Ok(rusqlite::types::ValueRef::Blob(bytes)) => {
                        Value::String(format!("<blob:{} bytes>", bytes.len()))
                    }
                    Err(_) => Value::Null,
                };
                map.insert(name.clone(), value);
            }
            result.push(map);
        }

        Ok(result)
    }

    fn execute(&self, sql: &str, params: &[Value]) -> Result<u64, DatabaseError> {
        let query_str = substitute_params(sql, params);

        self.conn
            .execute_batch(&query_str)
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        Ok(self.conn.changes())
    }

    fn ping(&self) -> Result<(), DatabaseError> {
        self.conn
            .execute_batch("SELECT 1")
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        Ok(())
    }

    fn close(&self) -> Result<(), DatabaseError> {
        Ok(())
    }

    fn get_table_schema(&self, table: &str) -> Result<TableSchema, DatabaseError> {
        let (schema_name, table_name) = parse_table_name(table);

        // SQLite uses PRAGMA table_info instead of information_schema
        let sql = format!("PRAGMA table_info('{}')", table_name.replace('\'', "''"));

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let mut columns = Vec::new();
        let mut rows = stmt
            .query([])
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        while let Some(row) = rows
            .next()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?
        {
            // PRAGMA table_info columns: cid, name, type, notnull, dflt_value, pk
            let cid: i64 = row
                .get(0)
                .map_err(|e| DatabaseError::QueryError(e.to_string()))?;
            let name: String = row
                .get(1)
                .map_err(|e| DatabaseError::QueryError(e.to_string()))?;
            let data_type: String = row
                .get(2)
                .map_err(|e| DatabaseError::QueryError(e.to_string()))?;
            let notnull: bool = row
                .get(3)
                .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

            columns.push(ColumnInfo {
                name,
                data_type,
                ordinal_position: (cid + 1) as usize, // 1-indexed
                is_nullable: !notnull,
            });
        }

        if columns.is_empty() {
            return Err(DatabaseError::NotFound);
        }

        Ok(TableSchema {
            schema_name,
            table_name,
            columns,
        })
    }
}
