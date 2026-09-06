use crate::validation::ValidationResult;
use polars::prelude::*;

/// Check for null values in specified columns
pub fn check_nulls(df: &DataFrame, columns: &[&str]) -> ValidationResult {
    let mut errors = Vec::new();

    for col_name in columns {
        if let Ok(series) = df.column(col_name) {
            let null_count = series.null_count();
            if null_count > 0 {
                errors.push(format!(
                    "Column '{}' contains {} null value(s)",
                    col_name, null_count
                ));
            }
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_nulls_no_nulls() {
        let df = df! {
            "id" => &[1i64, 2i64, 3i64],
            "name" => &["Alice", "Bob", "Charlie"],
        }
        .unwrap();

        let columns = vec!["id", "name"];
        let result = check_nulls(&df, &columns);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_check_nulls_with_nulls() {
        let df = df! {
            "id" => &[Some(1i64), None, Some(3i64)],
            "name" => &["Alice", "Bob", "Charlie"],
        }
        .unwrap();

        let columns = vec!["id"];
        let result = check_nulls(&df, &columns);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("id"));
        assert!(result.errors[0].contains("1 null"));
    }

    #[test]
    fn test_check_nulls_multiple_columns_with_nulls() {
        let df = df! {
            "id" => &[Some(1i64), None, Some(3i64)],
            "name" => &[Some("Alice"), Some("Bob"), None],
            "age" => &[25i64, 30i64, 35i64],
        }
        .unwrap();

        let columns = vec!["id", "name", "age"];
        let result = check_nulls(&df, &columns);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 2);
        assert!(result.errors.iter().any(|e| e.contains("id")));
        assert!(result.errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn test_check_nulls_empty_column_list() {
        let df = df! {
            "id" => &[Some(1i64), None, Some(3i64)],
        }
        .unwrap();

        let columns: Vec<&str> = vec![];
        let result = check_nulls(&df, &columns);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_check_nulls_nonexistent_column() {
        let df = df! {
            "id" => &[1i64, 2i64, 3i64],
        }
        .unwrap();

        let columns = vec!["id", "nonexistent"];
        let result = check_nulls(&df, &columns);

        // Should only check existing columns, nonexistent is silently skipped
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_check_nulls_all_nulls() {
        let df = df! {
            "id" => &[None::<i64>, None, None],
        }
        .unwrap();

        let columns = vec!["id"];
        let result = check_nulls(&df, &columns);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("3 null"));
    }
}
