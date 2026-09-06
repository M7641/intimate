//! `COPY` statement generation for bulk-loading a Parquet file into a table.
//!
//! Dialect-specific, so it lives next to the backend abstraction:
//! - DuckDB reads a path directly (local, or S3 via httpfs).
//! - Redshift (the Postgres backend) reads from S3 using an IAM role.
//! - Snowflake reaches S3 through a storage integration and maps Parquet columns
//!   to the table by name.

use crate::{DBType, DatabaseError};

/// Build a COPY statement that loads a Parquet file into a table.
///
/// Returns an error for backends that have no COPY-from-Parquet path, or whose
/// required configuration is missing — callers surface that rather than panic.
pub fn build_copy_sql(db_type: &DBType, table: &str, path: &str) -> Result<String, DatabaseError> {
    match db_type {
        #[cfg(feature = "duckdb")]
        DBType::DuckDb => Ok(format!("COPY {table} FROM '{path}' (FORMAT PARQUET)")),

        #[cfg(feature = "postgres")]
        DBType::Postgres => {
            // Redshift reads from S3 with an IAM role (from `REDSHIFT_IAM_ROLE`).
            let iam_role = std::env::var("REDSHIFT_IAM_ROLE").unwrap_or("tenant".to_string());
            Ok(format!(
                "COPY {table} FROM '{path}' IAM_ROLE '{iam_role}' FORMAT AS PARQUET"
            ))
        }

        #[cfg(feature = "snowflake")]
        DBType::Snowflake => {
            // Snowflake reaches the S3 location through a storage integration,
            // created out of band by infra — it has no safe default, so a missing
            // value is an error rather than a guess. `MATCH_BY_COLUMN_NAME` is what
            // makes a Parquet load populate the table's typed columns (matched by
            // name) instead of a single VARIANT column.
            let integration = std::env::var("SNOWFLAKE_STORAGE_INTEGRATION").map_err(|_| {
                DatabaseError::Other(
                    "SNOWFLAKE_STORAGE_INTEGRATION must be set to COPY from S3 into Snowflake"
                        .into(),
                )
            })?;
            Ok(format!(
                "COPY INTO {table} FROM '{path}' \
                 STORAGE_INTEGRATION = {integration} \
                 FILE_FORMAT = (TYPE = PARQUET) \
                 MATCH_BY_COLUMN_NAME = CASE_INSENSITIVE"
            ))
        }

        // Any other (feature-gated) backend has no COPY-from-Parquet support.
        #[allow(unreachable_patterns)]
        other => Err(DatabaseError::Other(format!(
            "COPY from Parquet is not supported for backend: {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "duckdb")]
    #[test]
    fn duckdb_copy_sql() {
        let sql = build_copy_sql(&DBType::DuckDb, "orders", "/data/orders.parquet").unwrap();
        assert_eq!(
            sql,
            "COPY orders FROM '/data/orders.parquet' (FORMAT PARQUET)"
        );
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn postgres_copy_sql() {
        let sql = build_copy_sql(
            &DBType::Postgres,
            "orders",
            "s3://bucket/tenant/orders.parquet",
        )
        .unwrap();
        // Should include IAM_ROLE from env (or default)
        assert!(sql.contains("COPY orders FROM"));
        assert!(sql.contains("IAM_ROLE"));
        assert!(sql.contains("FORMAT AS PARQUET"));
    }

    #[cfg(feature = "snowflake")]
    #[test]
    fn snowflake_copy_sql() {
        // Missing integration → explicit error (no guessed default).
        unsafe { std::env::remove_var("SNOWFLAKE_STORAGE_INTEGRATION") };
        assert!(build_copy_sql(&DBType::Snowflake, "stage.orders", "s3://b/k.parquet").is_err());

        // With the integration set, build the Snowflake `COPY INTO` form.
        unsafe { std::env::set_var("SNOWFLAKE_STORAGE_INTEGRATION", "MY_S3_INT") };
        let sql = build_copy_sql(&DBType::Snowflake, "stage.orders", "s3://b/k.parquet").unwrap();
        unsafe { std::env::remove_var("SNOWFLAKE_STORAGE_INTEGRATION") };

        assert!(sql.starts_with("COPY INTO stage.orders FROM 's3://b/k.parquet'"));
        assert!(sql.contains("STORAGE_INTEGRATION = MY_S3_INT"));
        assert!(sql.contains("FILE_FORMAT = (TYPE = PARQUET)"));
        assert!(sql.contains("MATCH_BY_COLUMN_NAME = CASE_INSENSITIVE"));
    }
}
