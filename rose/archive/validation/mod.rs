mod nulls;
mod range;
mod schema;
mod uniqueness;
mod valid_values;

pub use nulls::check_nulls;
pub use range::validate_range;
pub use schema::{dtype_to_string, validate_schema};
pub use uniqueness::validate_unique;
pub use valid_values::validate_valid_values;

use polars::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        self.valid = self.valid && other.valid;
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DataValidator;

impl DataValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validate a DataFrame against a schema with all validation checks
    /// This is the primary interface for validating data
    pub fn validate_dataframe(
        &self,
        df: &DataFrame,
        schema: &crate::schema::Schema,
    ) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Validate schema (column types)
        let schema_validation = validate_schema(df, &schema.get_column_types());
        result.merge(schema_validation);

        // If schema validation failed, no point in continuing
        if !result.valid {
            return result;
        }

        // Check for nulls in required columns
        let required_cols_vec = schema.get_required_columns();
        let required_cols: Vec<&str> = required_cols_vec.iter().map(|s| s.as_str()).collect();
        let null_validation = check_nulls(df, &required_cols);
        result.merge(null_validation);

        // Validate unique columns
        let unique_validation = validate_unique(df, &schema.get_unique_columns());
        result.merge(unique_validation);

        // Validate range constraints
        let range_validations = schema.get_range_validations();
        let range_validation = validate_range(df, &range_validations);
        result.merge(range_validation);

        // Validate accepted values
        let valid_values_validations = schema.get_valid_values_validations();
        let valid_values_refs: Vec<(&str, &[String])> = valid_values_validations
            .iter()
            .map(|(name, values)| (*name, values.as_slice()))
            .collect();
        let valid_values_validation = validate_valid_values(df, &valid_values_refs);
        result.merge(valid_values_validation);

        result
    }

    /// Validate a DataFrame against expected column types
    pub fn validate_schema(
        &self,
        df: &DataFrame,
        expected_schema: &std::collections::HashMap<String, String>,
    ) -> ValidationResult {
        validate_schema(df, expected_schema)
    }

    /// Check for null values in specified columns
    pub fn check_nulls(&self, df: &DataFrame, columns: &[&str]) -> ValidationResult {
        check_nulls(df, columns)
    }

    /// Validate unique values in specified columns
    pub fn validate_unique(&self, df: &DataFrame, columns: &[&str]) -> ValidationResult {
        validate_unique(df, columns)
    }

    /// Validate that numeric values fall within acceptable ranges
    pub fn validate_range(
        &self,
        df: &DataFrame,
        column_ranges: &[(&str, i64, i64)],
    ) -> ValidationResult {
        validate_range(df, column_ranges)
    }

    /// Validate that values are from a predefined list of acceptable values
    pub fn validate_valid_values(
        &self,
        df: &DataFrame,
        column_valid_values: &[(&str, &[String])],
    ) -> ValidationResult {
        validate_valid_values(df, column_valid_values)
    }
}

impl Default for DataValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{ColumnSchema, Validations};
    use std::collections::HashMap;

    #[test]
    fn test_validate_dataframe_success() {
        // Create a test DataFrame
        let df = df! {
            "id" => &[1i64, 2i64, 3i64],
            "name" => &["Alice", "Bob", "Charlie"],
        }
        .unwrap();

        // Create a schema
        let mut columns = HashMap::new();
        columns.insert(
            "id".to_string(),
            ColumnSchema {
                column_type: "int64".to_string(),
                nullable: false,
                description: None,
                validations: Some(Validations {
                    unique: Some(true),
                    range: None,
                    valid_values: None,
                }),
            },
        );
        columns.insert(
            "name".to_string(),
            ColumnSchema {
                column_type: "string".to_string(),
                nullable: false,
                description: None,
                validations: None,
            },
        );

        let schema = crate::schema::Schema {
            name: "test".to_string(),
            version: "1.0".to_string(),
            columns,
        };

        let validator = DataValidator::new();
        let result = validator.validate_dataframe(&df, &schema);

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_dataframe_with_errors() {
        // Create a test DataFrame with duplicate IDs
        let df = df! {
            "id" => &[1i64, 1i64, 2i64],
            "name" => &["Alice", "Bob", "Charlie"],
        }
        .unwrap();

        // Create a schema
        let mut columns = HashMap::new();
        columns.insert(
            "id".to_string(),
            ColumnSchema {
                column_type: "int64".to_string(),
                nullable: false,
                description: None,
                validations: Some(Validations {
                    unique: Some(true),
                    range: None,
                    valid_values: None,
                }),
            },
        );
        columns.insert(
            "name".to_string(),
            ColumnSchema {
                column_type: "string".to_string(),
                nullable: false,
                description: None,
                validations: None,
            },
        );

        let schema = crate::schema::Schema {
            name: "test".to_string(),
            version: "1.0".to_string(),
            columns,
        };

        let validator = DataValidator::new();
        let result = validator.validate_dataframe(&df, &schema);

        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_validation_result_merge() {
        let mut result1 = ValidationResult {
            valid: true,
            errors: vec!["error1".to_string()],
            warnings: vec!["warning1".to_string()],
        };

        let result2 = ValidationResult {
            valid: false,
            errors: vec!["error2".to_string()],
            warnings: vec!["warning2".to_string()],
        };

        result1.merge(result2);

        assert!(!result1.valid);
        assert_eq!(result1.errors.len(), 2);
        assert_eq!(result1.warnings.len(), 2);
    }

    #[test]
    fn test_validation_result_default() {
        let result = ValidationResult::default();
        assert!(result.valid);
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
    }
}
