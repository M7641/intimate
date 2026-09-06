use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use service_kit::error::AppError;
use service_kit::state::AppState;

/// Internal metadata columns excluded from column listings and filtering.
pub static EXCLUDED_COLUMNS: LazyLock<HashSet<&str>> = LazyLock::new(|| {
    HashSet::from([
        "row_hash",
        "load_timestamp",
        "batch_group_id",
        "snapshot_type",
        "source_system",
    ])
});

/// Regex for safe SQL identifiers: starts with letter or underscore, then
/// alphanumeric or underscores only.
static SAFE_IDENTIFIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap());

/// Validate that a table name is a safe SQL identifier.
///
/// Security is preserved by two layers:
/// 1. This regex prevents any SQL injection characters
/// 2. A valid-format but non-existent table simply returns empty results or a DB error
pub fn validate_table_name(table_name: &str) -> Result<(), AppError> {
    if !SAFE_IDENTIFIER.is_match(table_name) {
        return Err(AppError::Validation(format!(
            "Invalid table name: {table_name}"
        )));
    }
    Ok(())
}

/// Validate that a schema name is a safe SQL identifier.
pub fn validate_schema_name(schema: &str) -> Result<(), AppError> {
    if !SAFE_IDENTIFIER.is_match(schema) {
        return Err(AppError::Validation(format!(
            "Invalid schema name: {schema}"
        )));
    }
    Ok(())
}

/// Validate a column name: must match the safe identifier regex AND exist in the
/// provided set of real columns from the database schema.
pub fn validate_column_name(
    column_name: &str,
    valid_columns: &HashSet<String>,
) -> Result<(), AppError> {
    if !SAFE_IDENTIFIER.is_match(column_name) {
        return Err(AppError::Validation(format!(
            "Invalid column name: {column_name}"
        )));
    }
    if !valid_columns.contains(column_name) {
        return Err(AppError::Validation(format!(
            "Unknown column: {column_name}"
        )));
    }
    Ok(())
}

/// Escape single quotes in a SQL value to prevent injection.
pub fn escape_sql_value(value: &str) -> String {
    value.replace('\'', "''")
}

/// Build SQL to fetch valid column names for a table (used for validation).
pub fn build_valid_columns_query(schema: &str, table: &str) -> String {
    let schema_lower = schema.to_lowercase();
    let table_lower = table.to_lowercase();
    format!(
        "SELECT column_name \
         FROM information_schema.columns \
         WHERE lower(table_schema) = '{schema_lower}' \
         AND lower(table_name) = '{table_lower}'"
    )
}

/// Check whether a table has a `load_timestamp` column.
pub async fn table_has_load_timestamp(
    state: &AppState,
    schema: &str,
    table: &str,
) -> Result<bool, AppError> {
    let schema_lower = schema.to_lowercase();
    let table_lower = table.to_lowercase();
    let sql = format!(
        "SELECT 1 FROM information_schema.columns \
         WHERE lower(table_schema) = '{schema_lower}' \
         AND lower(table_name) = '{table_lower}' \
         AND lower(column_name) = 'load_timestamp' \
         LIMIT 1"
    );
    let rows = state.blocking_query(sql).await?;
    Ok(!rows.is_empty())
}

/// Build the WHERE clause for load_timestamp filtering.
///
/// When `has_timestamp` is false, returns `"WHERE 1=1"` — a safe no-op
/// that keeps any trailing `AND …` filter clauses syntactically valid.
///
/// When `has_timestamp` is true: if a timestamp is given, filters to that
/// exact value; otherwise falls back to the most recent timestamp.
pub fn build_timestamp_where_clause(
    schema: &str,
    table: &str,
    timestamp: Option<&str>,
    has_timestamp: bool,
) -> String {
    if !has_timestamp {
        return "WHERE 1=1".to_string();
    }
    match timestamp {
        Some(ts) => {
            let safe_ts = escape_sql_value(ts);
            format!("WHERE load_timestamp = '{safe_ts}'")
        }
        None => {
            format!("WHERE load_timestamp = (SELECT MAX(load_timestamp) FROM {schema}.\"{table}\")")
        }
    }
}

/// Check whether a Redshift/information_schema data type is numeric.
pub fn is_numeric_data_type(data_type: &str) -> bool {
    matches!(
        data_type.to_lowercase().as_str(),
        "integer"
            | "int"
            | "int2"
            | "int4"
            | "int8"
            | "smallint"
            | "bigint"
            | "numeric"
            | "decimal"
            | "real"
            | "float"
            | "float4"
            | "float8"
            | "double precision"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_table_names() {
        assert!(validate_table_name("users").is_ok());
        assert!(validate_table_name("my_table_123").is_ok());
        assert!(validate_table_name("_private").is_ok());
        assert!(validate_table_name("A").is_ok());
    }

    #[test]
    fn invalid_table_names() {
        assert!(validate_table_name("my-table").is_err());
        assert!(validate_table_name("123table").is_err());
        assert!(validate_table_name("my table").is_err());
        assert!(validate_table_name("").is_err());
        assert!(validate_table_name("users; DROP TABLE--").is_err());
        assert!(validate_table_name("table'name").is_err());
    }

    #[test]
    fn valid_schema_names() {
        assert!(validate_schema_name("public").is_ok());
        assert!(validate_schema_name("stage").is_ok());
        assert!(validate_schema_name("my_schema_v2").is_ok());
    }

    #[test]
    fn invalid_schema_names() {
        assert!(validate_schema_name("my schema").is_err());
        assert!(validate_schema_name("schema;drop").is_err());
    }

    #[test]
    fn column_name_must_exist_in_set() {
        let valid = HashSet::from(["id".to_string(), "name".to_string(), "email".to_string()]);
        assert!(validate_column_name("id", &valid).is_ok());
        assert!(validate_column_name("name", &valid).is_ok());
        assert!(validate_column_name("nonexistent", &valid).is_err());
    }

    #[test]
    fn column_name_regex_enforced() {
        let valid = HashSet::from(["bad-col".to_string()]);
        // Even if the column exists in the set, the regex rejects it
        assert!(validate_column_name("bad-col", &valid).is_err());
    }

    #[test]
    fn escape_sql_value_handles_quotes() {
        assert_eq!(escape_sql_value("hello"), "hello");
        assert_eq!(escape_sql_value("it's"), "it''s");
        assert_eq!(escape_sql_value("a''b"), "a''''b");
    }

    #[test]
    fn is_numeric_detects_types() {
        assert!(is_numeric_data_type("integer"));
        assert!(is_numeric_data_type("BIGINT"));
        assert!(is_numeric_data_type("float8"));
        assert!(is_numeric_data_type("double precision"));
        assert!(!is_numeric_data_type("varchar"));
        assert!(!is_numeric_data_type("text"));
        assert!(!is_numeric_data_type("boolean"));
    }
}
