use crate::validation::ValidationResult;
use polars::prelude::*;

/// Validate that numeric values in specified columns fall within acceptable ranges
///
/// Takes a DataFrame and a list of tuples (column_name, min, max)
pub fn validate_range(df: &DataFrame, column_ranges: &[(&str, i64, i64)]) -> ValidationResult {
    let mut errors = Vec::new();

    for (col_name, min_val, max_val) in column_ranges {
        if let Ok(series) = df.column(col_name) {
            match series.dtype() {
                DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64 => {
                    // Convert to i64 for comparison
                    if let Ok(int_series) = series.cast(&DataType::Int64) {
                        let ca = int_series.i64().unwrap();

                        // Check for values outside range (excluding nulls)
                        for (idx, opt_val) in ca.iter().enumerate() {
                            if let Some(val) = opt_val {
                                if val < *min_val {
                                    errors.push(format!(
                                        "Column '{}' row {} has value {} which is less than minimum {}",
                                        col_name, idx, val, min_val
                                    ));
                                } else if val > *max_val {
                                    errors.push(format!(
                                        "Column '{}' row {} has value {} which is greater than maximum {}",
                                        col_name, idx, val, max_val
                                    ));
                                }
                            }
                        }
                    }
                }
                DataType::Float32 | DataType::Float64 => {
                    // Convert to f64 for comparison
                    if let Ok(float_series) = series.cast(&DataType::Float64) {
                        let ca = float_series.f64().unwrap();
                        let min_f64 = *min_val as f64;
                        let max_f64 = *max_val as f64;

                        for (idx, opt_val) in ca.iter().enumerate() {
                            if let Some(val) = opt_val {
                                if val < min_f64 {
                                    errors.push(format!(
                                        "Column '{}' row {} has value {} which is less than minimum {}",
                                        col_name, idx, val, min_val
                                    ));
                                } else if val > max_f64 {
                                    errors.push(format!(
                                        "Column '{}' row {} has value {} which is greater than maximum {}",
                                        col_name, idx, val, max_val
                                    ));
                                }
                            }
                        }
                    }
                }
                _ => {
                    errors.push(format!(
                        "Column '{}' has type {:?} which is not numeric, cannot validate range",
                        col_name,
                        series.dtype()
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
    fn test_validate_range_within_bounds() {
        let df = df! {
            "age" => &[25i64, 30i64, 35i64, 40i64],
        }
        .unwrap();

        let ranges = vec![("age", 18, 65)];
        let result = validate_range(&df, &ranges);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_range_below_minimum() {
        let df = df! {
            "age" => &[15i64, 25i64, 30i64],
        }
        .unwrap();

        let ranges = vec![("age", 18, 65)];
        let result = validate_range(&df, &ranges);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("age"));
        assert!(result.errors[0].contains("row 0"));
        assert!(result.errors[0].contains("15"));
        assert!(result.errors[0].contains("less than minimum 18"));
    }

    #[test]
    fn test_validate_range_above_maximum() {
        let df = df! {
            "age" => &[25i64, 30i64, 70i64],
        }
        .unwrap();

        let ranges = vec![("age", 18, 65)];
        let result = validate_range(&df, &ranges);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("age"));
        assert!(result.errors[0].contains("row 2"));
        assert!(result.errors[0].contains("70"));
        assert!(result.errors[0].contains("greater than maximum 65"));
    }

    #[test]
    fn test_validate_range_multiple_violations() {
        let df = df! {
            "age" => &[15i64, 25i64, 70i64, 100i64],
        }
        .unwrap();

        let ranges = vec![("age", 18, 65)];
        let result = validate_range(&df, &ranges);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 3);
    }

    #[test]
    fn test_validate_range_exact_bounds() {
        let df = df! {
            "age" => &[18i64, 30i64, 65i64],
        }
        .unwrap();

        let ranges = vec![("age", 18, 65)];
        let result = validate_range(&df, &ranges);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_range_with_nulls() {
        let df = df! {
            "age" => &[Some(25i64), None, Some(30i64)],
        }
        .unwrap();

        let ranges = vec![("age", 18, 65)];
        let result = validate_range(&df, &ranges);

        // Nulls should be ignored in range validation
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_range_multiple_columns() {
        let df = df! {
            "age" => &[25i64, 30i64, 35i64],
            "score" => &[85i64, 90i64, 95i64],
        }
        .unwrap();

        let ranges = vec![("age", 18, 65), ("score", 0, 100)];
        let result = validate_range(&df, &ranges);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_range_float_values() {
        let df = df! {
            "price" => &[10.5f64, 20.0f64, 30.75f64],
        }
        .unwrap();

        let ranges = vec![("price", 10, 50)];
        let result = validate_range(&df, &ranges);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_range_float_out_of_bounds() {
        let df = df! {
            "price" => &[5.5f64, 20.0f64, 55.75f64],
        }
        .unwrap();

        let ranges = vec![("price", 10, 50)];
        let result = validate_range(&df, &ranges);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 2);
    }

    #[test]
    fn test_validate_range_non_numeric_column() {
        let df = df! {
            "name" => &["Alice", "Bob", "Charlie"],
        }
        .unwrap();

        let ranges = vec![("name", 0, 100)];
        let result = validate_range(&df, &ranges);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("not numeric"));
    }

    #[test]
    fn test_validate_range_empty_list() {
        let df = df! {
            "age" => &[25i64, 30i64],
        }
        .unwrap();

        let ranges: Vec<(&str, i64, i64)> = vec![];
        let result = validate_range(&df, &ranges);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_range_nonexistent_column() {
        let df = df! {
            "age" => &[25i64, 30i64],
        }
        .unwrap();

        let ranges = vec![("nonexistent", 0, 100)];
        let result = validate_range(&df, &ranges);

        // Column doesn't exist, so no errors are generated
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_range_int32() {
        let df = df! {
            "value" => &[10i32, 20i32, 30i32],
        }
        .unwrap();

        let ranges = vec![("value", 5, 50)];
        let result = validate_range(&df, &ranges);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }
}
