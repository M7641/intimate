//! INSERT functionality for the database abstraction layer.

use crate::schema::normalize_column_name;
use crate::sql::value_to_sql_string;
use crate::traits::DatabaseError;
use serde::Serialize;
use serde_json::Value;

const DEFAULT_BATCH_SIZE: usize = 1000;

/// Serialize a struct to a map of column names to JSON values.
fn struct_to_columns<T: Serialize>(data: &T) -> Result<Vec<(String, Value)>, DatabaseError> {
    let value =
        serde_json::to_value(data).map_err(|e| DatabaseError::SerializationError(e.to_string()))?;

    let obj = value.as_object().ok_or_else(|| {
        DatabaseError::ValidationError("Data must serialize to a JSON object".to_string())
    })?;

    if obj.is_empty() {
        return Err(DatabaseError::ValidationError(
            "Cannot insert empty struct".to_string(),
        ));
    }

    let columns: Vec<(String, Value)> = obj
        .iter()
        .map(|(k, v)| (normalize_column_name(k), v.clone()))
        .collect();

    Ok(columns)
}

/// Build batched INSERT statements for multiple rows with default batch size of 1000.
pub fn build_insert_sql<T: Serialize>(
    table: &str,
    data: &[T],
) -> Result<Vec<String>, DatabaseError> {
    build_insert_sql_with_batch_size(table, data, DEFAULT_BATCH_SIZE)
}

/// Build batched INSERT statements for multiple rows with configurable batch size.
pub fn build_insert_sql_with_batch_size<T: Serialize>(
    table: &str,
    data: &[T],
    batch_size: usize,
) -> Result<Vec<String>, DatabaseError> {
    validate_table_name(table)?;

    if data.is_empty() {
        return Ok(vec![]);
    }

    let batch_size = if batch_size == 0 {
        data.len()
    } else {
        batch_size
    };

    // Get column names from the first item (reuse its values to avoid double serialization)
    let first_columns = struct_to_columns(&data[0])?;
    let col_names: Vec<&str> = first_columns.iter().map(|(k, _)| k.as_str()).collect();
    let col_list = col_names.join(", ");

    let mut statements = Vec::new();
    let mut first_item_columns: Option<Vec<(String, Value)>> = Some(first_columns);

    for chunk in data.chunks(batch_size) {
        let mut value_groups = Vec::new();

        for item in chunk {
            let columns = if first_item_columns.is_some() {
                first_item_columns.take().unwrap()
            } else {
                struct_to_columns(item)?
            };
            let values: Vec<String> = columns
                .iter()
                .map(|(_, v)| value_to_sql_string(v))
                .collect();
            value_groups.push(format!("({})", values.join(", ")));
        }

        statements.push(format!(
            "INSERT INTO {} ({}) VALUES {}",
            table,
            col_list,
            value_groups.join(", ")
        ));
    }

    Ok(statements)
}

pub fn validate_table_name(table: &str) -> Result<(), DatabaseError> {
    if table.trim().is_empty() {
        return Err(DatabaseError::ValidationError(
            "Table name cannot be empty".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct User {
        id: i64,
        name: String,
        email: String,
    }

    #[derive(Serialize)]
    struct UserWithNull {
        id: i64,
        name: String,
        email: Option<String>,
    }

    #[test]
    fn test_build_insert_sql_single() {
        let users = vec![User {
            id: 1,
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        }];
        let statements = build_insert_sql("users", &users).unwrap();

        assert_eq!(statements.len(), 1);
        assert!(statements[0].starts_with("INSERT INTO users ("));
        assert!(statements[0].contains("1"));
        assert!(statements[0].contains("'Alice'"));
    }

    #[test]
    fn test_build_insert_sql_multiple() {
        let users = vec![
            User {
                id: 1,
                name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
            },
            User {
                id: 2,
                name: "Bob".to_string(),
                email: "bob@example.com".to_string(),
            },
        ];
        let statements = build_insert_sql("users", &users).unwrap();

        assert_eq!(statements.len(), 1);
        assert!(statements[0].contains("'Alice'"));
        assert!(statements[0].contains("'Bob'"));
    }

    #[test]
    fn test_build_insert_sql_with_null() {
        let users = vec![UserWithNull {
            id: 1,
            name: "Bob".to_string(),
            email: None,
        }];
        let statements = build_insert_sql("users", &users).unwrap();

        assert!(statements[0].contains("NULL"));
    }

    #[test]
    fn test_build_insert_sql_escapes_quotes() {
        let users = vec![User {
            id: 1,
            name: "O'Brien".to_string(),
            email: "test@example.com".to_string(),
        }];
        let statements = build_insert_sql("users", &users).unwrap();

        assert!(statements[0].contains("'O''Brien'"));
    }

    #[test]
    fn test_build_insert_sql_empty_returns_empty() {
        let users: Vec<User> = vec![];
        let statements = build_insert_sql("users", &users).unwrap();
        assert!(statements.is_empty());
    }

    #[test]
    fn test_empty_table_name_error() {
        let users = vec![User {
            id: 1,
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        }];
        let result = build_insert_sql("", &users);
        assert!(matches!(result, Err(DatabaseError::ValidationError(_))));
    }

    #[test]
    fn test_build_insert_sql_custom_batch_size() {
        let users: Vec<User> = (0..5)
            .map(|i| User {
                id: i,
                name: format!("User{}", i),
                email: format!("user{}@example.com", i),
            })
            .collect();

        // Batch size of 2 should produce 3 statements (2, 2, 1)
        let statements = build_insert_sql_with_batch_size("users", &users, 2).unwrap();
        assert_eq!(statements.len(), 3);

        // Batch size of 0 means no batching (single statement)
        let statements = build_insert_sql_with_batch_size("users", &users, 0).unwrap();
        assert_eq!(statements.len(), 1);
    }
}
