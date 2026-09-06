use crate::validation::ValidationResult;
use polars::prelude::*;

/// Validate unique values in specified columns
pub fn validate_unique(df: &DataFrame, columns: &[&str]) -> ValidationResult {
    let mut errors = Vec::new();

    for col_name in columns {
        if let Ok(series) = df.column(col_name) {
            let unique_count = series.n_unique().unwrap_or(0);
            let total_count = series.len();

            if unique_count < total_count {
                errors.push(format!(
                    "Column '{}' should have unique values but has {} unique out of {} total",
                    col_name, unique_count, total_count
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
    fn test_validate_unique_all_unique() {
        let df = df! {
            "id" => &[1i64, 2i64, 3i64, 4i64],
            "email" => &["a@test.com", "b@test.com", "c@test.com", "d@test.com"],
        }
        .unwrap();

        let columns = vec!["id", "email"];
        let result = validate_unique(&df, &columns);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_unique_with_duplicates() {
        let df = df! {
            "id" => &[1i64, 2i64, 2i64, 3i64],
        }
        .unwrap();

        let columns = vec!["id"];
        let result = validate_unique(&df, &columns);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("id"));
        assert!(result.errors[0].contains("3 unique out of 4 total"));
    }

    #[test]
    fn test_validate_unique_multiple_columns_with_duplicates() {
        let df = df! {
            "id" => &[1i64, 2i64, 2i64, 3i64],
            "email" => &["a@test.com", "b@test.com", "b@test.com", "c@test.com"],
        }
        .unwrap();

        let columns = vec!["id", "email"];
        let result = validate_unique(&df, &columns);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 2);
        assert!(result.errors.iter().any(|e| e.contains("id")));
        assert!(result.errors.iter().any(|e| e.contains("email")));
    }

    #[test]
    fn test_validate_unique_empty_column_list() {
        let df = df! {
            "id" => &[1i64, 2i64, 2i64],
        }
        .unwrap();

        let columns: Vec<&str> = vec![];
        let result = validate_unique(&df, &columns);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_unique_nonexistent_column() {
        let df = df! {
            "id" => &[1i64, 2i64, 3i64],
        }
        .unwrap();

        let columns = vec!["id", "nonexistent"];
        let result = validate_unique(&df, &columns);

        // Should only check existing columns, nonexistent is silently skipped
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_unique_all_same_value() {
        let df = df! {
            "id" => &[1i64, 1i64, 1i64, 1i64],
        }
        .unwrap();

        let columns = vec!["id"];
        let result = validate_unique(&df, &columns);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("1 unique out of 4 total"));
    }

    #[test]
    fn test_validate_unique_single_row() {
        let df = df! {
            "id" => &[1i64],
        }
        .unwrap();

        let columns = vec!["id"];
        let result = validate_unique(&df, &columns);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_unique_empty_dataframe() {
        let df = df! {
            "id" => &[] as &[i64],
        }
        .unwrap();

        let columns = vec!["id"];
        let result = validate_unique(&df, &columns);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_unique_string_duplicates() {
        let df = df! {
            "name" => &["Alice", "Bob", "Alice", "Charlie"],
        }
        .unwrap();

        let columns = vec!["name"];
        let result = validate_unique(&df, &columns);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("name"));
        assert!(result.errors[0].contains("3 unique out of 4 total"));
    }

    #[test]
    fn test_validate_unique_with_nulls() {
        let df = df! {
            "id" => &[Some(1i64), Some(2i64), None, Some(1i64)],
        }
        .unwrap();

        let columns = vec!["id"];
        let result = validate_unique(&df, &columns);

        assert!(!result.valid);
        // Should detect duplicates even with nulls present
        assert_eq!(result.errors.len(), 1);
    }
}
