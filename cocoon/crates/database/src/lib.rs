// The crate has no default backend (see Cargo.toml). Building it with none
// enabled leaves empty connector enums, so fail early with a clear message
// instead of a confusing "non-exhaustive match" error.
#[cfg(not(any(
    feature = "duckdb",
    feature = "sqlite",
    feature = "postgres",
    feature = "snowflake",
)))]
compile_error!(
    "database needs at least one backend feature: duckdb, sqlite, postgres, postgres-async, or snowflake"
);

pub mod actions;
pub mod connectors;
pub mod ident;
pub mod pool;
pub mod schema;
pub mod sql;
pub mod traits;

// -- Re-exports --

pub use actions::{
    ColumnMapping, DBActions, DBType, DatabaseConfig, InsertColumns, MergeCondition, MergeConfig,
    UpdateColumns, build_copy_sql, build_insert_sql, build_merge_sql,
};
pub use ident::{Identifier, IdentifierError};
pub use pool::ApiDbActions;
pub use schema::{ColumnInfo, TableSchema};
pub use traits::{Database, DatabaseError, QueryResult, Row};

#[cfg(feature = "duckdb")]
pub use connectors::duckdb::{DuckDBDatabase, DuckDbConfig};

#[cfg(feature = "sqlite")]
pub use connectors::sqlite::{SqliteConfig, SqliteDatabase};

#[cfg(feature = "postgres")]
pub use connectors::postgres::{PostgresConfig, PostgresDatabase};

#[cfg(feature = "snowflake")]
pub use connectors::snowflake::{SnowflakeConfig, SnowflakeDatabase, enable_smallest_warehouse};
