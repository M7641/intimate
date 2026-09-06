use crate::validation::ValidationResult;
use polars::prelude::*;
use std::collections::HashMap;

/// Validate a DataFrame against expected column types
pub fn validate_schema(
    df: &DataFrame,
    expected_schema: &HashMap<String, String>,
) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for (col_name, expected_type) in expected_schema {
        match df.column(col_name) {
            Ok(series) => {
                let actual_type = dtype_to_string(series.dtype());
                if &actual_type != expected_type {
                    errors.push(format!(
                        "Column '{}' has type '{}' but expected '{}'",
                        col_name, actual_type, expected_type
                    ));
                }
            }
            Err(_) => {
                errors.push(format!("Column '{}' not found in DataFrame", col_name));
            }
        }
    }

    // Check for extra columns not in schema
    for col_name in df.get_column_names() {
        let col_name_str = col_name.to_string();
        if !expected_schema.contains_key(&col_name_str) {
            warnings.push(format!(
                "Column '{}' exists in data but not in schema",
                col_name
            ));
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

/// Convert polars DataType to string representation
pub fn dtype_to_string(dtype: &DataType) -> String {
    match dtype {
        DataType::Boolean => "boolean".to_string(),
        DataType::UInt8 => "uint8".to_string(),
        DataType::UInt16 => "uint16".to_string(),
        DataType::UInt32 => "uint32".to_string(),
        DataType::UInt64 => "uint64".to_string(),
        DataType::Int8 => "int8".to_string(),
        DataType::Int16 => "int16".to_string(),
        DataType::Int32 => "int32".to_string(),
        DataType::Int64 => "int64".to_string(),
        DataType::Float32 => "float32".to_string(),
        DataType::Float64 => "float64".to_string(),
        DataType::String => "string".to_string(),
        DataType::Date => "date".to_string(),
        DataType::Datetime(_, _) => "datetime".to_string(),
        _ => format!("{:?}", dtype).to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_schema_success() {
        let df = df! {
            "id" => &[1i64, 2i64, 3i64],
            "name" => &["Alice", "Bob", "Charlie"],
            "age" => &[25i64, 30i64, 35i64],
        }
        .unwrap();

        let mut expected = HashMap::new();
        expected.insert("id".to_string(), "int64".to_string());
        expected.insert("name".to_string(), "string".to_string());
        expected.insert("age".to_string(), "int64".to_string());

        let result = validate_schema(&df, &expected);

        assert!(result.valid);
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validate_schema_type_mismatch() {
        let df = df! {
            "id" => &[1i64, 2i64, 3i64],
            "name" => &["Alice", "Bob", "Charlie"],
        }
        .unwrap();

        let mut expected = HashMap::new();
        expected.insert("id".to_string(), "string".to_string()); // Wrong type
        expected.insert("name".to_string(), "string".to_string());

        let result = validate_schema(&df, &expected);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("id"));
        assert!(result.errors[0].contains("int64"));
        assert!(result.errors[0].contains("string"));
    }

    #[test]
    fn test_validate_schema_missing_column() {
        let df = df! {
            "id" => &[1i64, 2i64, 3i64],
        }
        .unwrap();

        let mut expected = HashMap::new();
        expected.insert("id".to_string(), "int64".to_string());
        expected.insert("name".to_string(), "string".to_string()); // Missing in df

        let result = validate_schema(&df, &expected);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("name"));
        assert!(result.errors[0].contains("not found"));
    }

    #[test]
    fn test_validate_schema_extra_column() {
        let df = df! {
            "id" => &[1i64, 2i64, 3i64],
            "name" => &["Alice", "Bob", "Charlie"],
            "extra" => &[1.0, 2.0, 3.0],
        }
        .unwrap();

        let mut expected = HashMap::new();
        expected.insert("id".to_string(), "int64".to_string());
        expected.insert("name".to_string(), "string".to_string());

        let result = validate_schema(&df, &expected);

        assert!(result.valid); // Still valid, just has warnings
        assert!(result.errors.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("extra"));
    }

    #[test]
    fn test_dtype_to_string() {
        assert_eq!(dtype_to_string(&DataType::Int64), "int64");
        assert_eq!(dtype_to_string(&DataType::String), "string");
        assert_eq!(dtype_to_string(&DataType::Float32), "float32");
        assert_eq!(dtype_to_string(&DataType::Boolean), "boolean");
        assert_eq!(dtype_to_string(&DataType::Date), "date");
    }

    #[test]
    fn test_validate_schema_multiple_errors() {
        let df = df! {
            "id" => &[1i64, 2i64, 3i64],
        }
        .unwrap();

        let mut expected = HashMap::new();
        expected.insert("id".to_string(), "string".to_string()); // Wrong type
        expected.insert("name".to_string(), "string".to_string()); // Missing column

        let result = validate_schema(&df, &expected);

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 2);
    }
}
