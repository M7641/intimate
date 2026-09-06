pub mod external;
pub mod in_memory;

// Convenience re-exports at the connectors level
#[cfg(feature = "duckdb")]
pub use in_memory::duckdb;

#[cfg(feature = "sqlite")]
pub use in_memory::sqlite;

#[cfg(feature = "postgres")]
pub use external::postgres;

#[cfg(feature = "postgres-async")]
pub use external::postgres_async;

#[cfg(feature = "snowflake")]
pub use external::snowflake;
