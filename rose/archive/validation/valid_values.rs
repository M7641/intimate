use crate::validation::ValidationResult;
use polars::prelude::*;

/// Validate that values in specified columns are from a predefined list of acceptable values
///
/// Takes a DataFrame and a list of tuples (column_name, valid_values)
pub fn validate_valid_values(
    df: &DataFrame,
    column_valid_values: &[(&str, &[String])],
) -> ValidationResult {
    let mut errors = Vec::new();

    for (col_name, valid_values) in column_valid_values {
        if let Ok(series) = df.column(col_name) {
            match series.dtype() {
                DataType::String => {
                    let ca = series.str().unwrap();

                    for (idx, opt_val) in ca.iter().enumerate() {
                        if let Some(val) = opt_val {
                            if !valid_values.contains(&val.to_string()) {
                                errors.push(format!(
                                    "Column '{}' row {} has value '{}' which is not in the list of valid values: [{}]",
                                    col_name,
                                    idx,
                                    val,
                                    valid_values.join(", ")
                                ));
                            }
                        }
                    }
                }
                _ => {
                    errors.push(format!(
                        "Column '{}' has type {:?} which is not String, cannot validate against valid values",
                        col_name, series.dtype()
                    ));
                }
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
    fn test_validate_valid_values_all_valid() {
        let df = df! {
            "status" => &["active", "pending", "active"],
        }
        .unwrap();

        let valid_values = vec![
            "active".to_string(),
            "pending".to_string(),
            "completed".to_string(),
        ];
        let column_values = vec![("status", valid_values.as_slice())];
        let result = validate_valid_values(&df, &column_values);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_valid_values_with_invalid() {
        let df = df! {
            "status" => &["active", "invalid", "pending"],
        }
        .unwrap();

        let valid_values = vec![
            "active".to_string(),
            "pending".to_string(),
            "completed".to_string(),
        ];
        let column_values = vec![("status", valid_values.as_slice())];
        let result = validate_valid_values(&df, &column_values);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("status"));
        assert!(result.errors[0].contains("row 1"));
        assert!(result.errors[0].contains("invalid"));
    }

    #[test]
    fn test_validate_valid_values_multiple_invalid() {
        let df = df! {
            "status" => &["active", "invalid", "wrong", "pending"],
        }
        .unwrap();

        let valid_values = vec![
            "active".to_string(),
            "pending".to_string(),
            "completed".to_string(),
        ];
        let column_values = vec![("status", valid_values.as_slice())];
        let result = validate_valid_values(&df, &column_values);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 2);
        assert!(result.errors.iter().any(|e| e.contains("invalid")));
        assert!(result.errors.iter().any(|e| e.contains("wrong")));
    }

    #[test]
    fn test_validate_valid_values_with_nulls() {
        let df = df! {
            "status" => &[Some("active"), None, Some("pending")],
        }
        .unwrap();

        let valid_values = vec![
            "active".to_string(),
            "pending".to_string(),
            "completed".to_string(),
        ];
        let column_values = vec![("status", valid_values.as_slice())];
        let result = validate_valid_values(&df, &column_values);

        // Nulls should be ignored in valid values validation
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_valid_values_empty_list() {
        let df = df! {
            "status" => &["active", "pending"],
        }
        .unwrap();

        let column_values: Vec<(&str, &[String])> = vec![];
        let result = validate_valid_values(&df, &column_values);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_valid_values_nonexistent_column() {
        let df = df! {
            "status" => &["active", "pending"],
        }
        .unwrap();

        let valid_values = vec!["active".to_string(), "pending".to_string()];
        let column_values = vec![("nonexistent", valid_values.as_slice())];
        let result = validate_valid_values(&df, &column_values);

        // Column doesn't exist, so no errors are generated
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_valid_values_non_string_column() {
        let df = df! {
            "id" => &[1i64, 2i64, 3i64],
        }
        .unwrap();

        let valid_values = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let column_values = vec![("id", valid_values.as_slice())];
        let result = validate_valid_values(&df, &column_values);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("not String"));
    }

    #[test]
    fn test_validate_valid_values_multiple_columns() {
        let df = df! {
            "status" => &["active", "pending", "completed"],
            "priority" => &["high", "low", "medium"],
        }
        .unwrap();

        let status_values = vec![
            "active".to_string(),
            "pending".to_string(),
            "completed".to_string(),
        ];
        let priority_values = vec!["high".to_string(), "medium".to_string(), "low".to_string()];
        let column_values = vec![
            ("status", status_values.as_slice()),
            ("priority", priority_values.as_slice()),
        ];
        let result = validate_valid_values(&df, &column_values);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_valid_values_case_sensitive() {
        let df = df! {
            "status" => &["Active", "pending"],
        }
        .unwrap();

        let valid_values = vec!["active".to_string(), "pending".to_string()];
        let column_values = vec![("status", valid_values.as_slice())];
        let result = validate_valid_values(&df, &column_values);

        // Should be case-sensitive by default
        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("Active"));
    }

    #[test]
    fn test_validate_valid_values_single_value() {
        let df = df! {
            "status" => &["active", "active", "active"],
        }
        .unwrap();

        let valid_values = vec!["active".to_string()];
        let column_values = vec![("status", valid_values.as_slice())];
        let result = validate_valid_values(&df, &column_values);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_valid_values_empty_string() {
        let df = df! {
            "status" => &["active", "", "pending"],
        }
        .unwrap();

        let valid_values = vec!["active".to_string(), "".to_string(), "pending".to_string()];
        let column_values = vec![("status", valid_values.as_slice())];
        let result = validate_valid_values(&df, &column_values);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_valid_values_special_characters() {
        let df = df! {
            "status" => &["active-1", "pending@2", "completed#3"],
        }
        .unwrap();

        let valid_values = vec![
            "active-1".to_string(),
            "pending@2".to_string(),
            "completed#3".to_string(),
        ];
        let column_values = vec![("status", valid_values.as_slice())];
        let result = validate_valid_values(&df, &column_values);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }
}
