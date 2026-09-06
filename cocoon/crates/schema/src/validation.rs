use polars::prelude::*;
use std::collections::HashMap;
use std::fmt;

use super::types::{ExtraColumnPolicy, SchemaError, TableSchema};

// ── Report types ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub missing_columns: Vec<String>,
    pub extra_columns: Vec<String>,
    pub type_mismatches: Vec<TypeMismatch>,
    pub order_mismatches: Vec<OrderMismatch>,
    pub info_extra_columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TypeMismatch {
    pub column: String,
    pub expected: DataType,
    pub actual: DataType,
}

#[derive(Debug, Clone)]
pub struct OrderMismatch {
    pub column: String,
    pub expected_position: usize,
    pub actual_position: usize,
}

impl ValidationReport {
    pub(crate) fn is_valid(&self) -> bool {
        self.missing_columns.is_empty()
            && self.extra_columns.is_empty()
            && self.type_mismatches.is_empty()
            && self.order_mismatches.is_empty()
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for col in &self.missing_columns {
            writeln!(f, "  missing column: {col}")?;
        }
        for col in &self.extra_columns {
            writeln!(f, "  extra column: {col}")?;
        }
        for m in &self.type_mismatches {
            writeln!(
                f,
                "  type mismatch on '{}': expected {:?}, got {:?}",
                m.column, m.expected, m.actual
            )?;
        }
        for m in &self.order_mismatches {
            writeln!(
                f,
                "  order mismatch on '{}': expected position {}, got {}",
                m.column, m.expected_position, m.actual_position
            )?;
        }
        for col in &self.info_extra_columns {
            writeln!(f, "  info: extra column accepted: {col}")?;
        }
        Ok(())
    }
}

// ── Validator ──────────────────────────────────────────────────────

/// Validate a DataFrame against a `TableSchema`.
///
/// Checks column presence, absence of extras, type compatibility,
/// and column ordering. All mismatches are collected into a single
/// `ValidationReport` so users can fix everything in one pass.
///
/// When the schema's `extra_column_policy` is `Allow` (e.g. satellites),
/// extra columns are tracked in `info_extra_columns` instead of `extra_columns`,
/// and order mismatches are skipped (satellites are flexible).
pub fn validate_dataframe(
    schema: &TableSchema,
    df: &DataFrame,
) -> Result<ValidationReport, SchemaError> {
    let df_names: Vec<String> = df
        .get_column_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let df_lookup: HashMap<&str, (usize, &DataType)> = df
        .get_columns()
        .iter()
        .enumerate()
        .map(|(i, col)| (col.name().as_str(), (i, col.dtype())))
        .collect();

    let flexible = schema.extra_column_policy == ExtraColumnPolicy::Allow;

    let mut report = ValidationReport {
        missing_columns: Vec::new(),
        extra_columns: Vec::new(),
        type_mismatches: Vec::new(),
        order_mismatches: Vec::new(),
        info_extra_columns: Vec::new(),
    };

    // Check each expected column
    for (expected_pos, col_def) in schema.columns.iter().enumerate() {
        match df_lookup.get(col_def.name.as_str()) {
            Some(&(actual_pos, actual_dtype)) => {
                if *actual_dtype != col_def.dtype {
                    report.type_mismatches.push(TypeMismatch {
                        column: col_def.name.clone(),
                        expected: col_def.dtype.clone(),
                        actual: actual_dtype.clone(),
                    });
                }
                if !flexible && actual_pos != expected_pos {
                    report.order_mismatches.push(OrderMismatch {
                        column: col_def.name.clone(),
                        expected_position: expected_pos,
                        actual_position: actual_pos,
                    });
                }
            }
            None => {
                report.missing_columns.push(col_def.name.clone());
            }
        }
    }

    // Check for extra columns not in schema
    let expected_names: HashMap<&str, ()> = schema
        .columns
        .iter()
        .map(|c| (c.name.as_str(), ()))
        .collect();

    for name in &df_names {
        if !expected_names.contains_key(name.as_str()) {
            if flexible {
                report.info_extra_columns.push(name.clone());
            } else {
                report.extra_columns.push(name.clone());
            }
        }
    }

    if report.is_valid() {
        Ok(report)
    } else {
        Err(SchemaError::ValidationFailed(report))
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColumnDef;

    fn make_schema(cols: Vec<(&str, DataType)>) -> TableSchema {
        TableSchema {
            name: "test".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: crate::CompatibilityMode::None,
            keys: None,
            columns: cols
                .into_iter()
                .map(|(name, dtype)| ColumnDef {
                    name: name.to_string(),
                    dtype,
                    constraints: None,
                })
                .collect(),
            dv_metadata: None,
            extra_column_policy: crate::ExtraColumnPolicy::default(),
        }
    }

    #[test]
    fn valid_dataframe_passes() {
        let schema = make_schema(vec![("id", DataType::Int64), ("name", DataType::String)]);
        let df = df! {
            "id" => &[1i64, 2, 3],
            "name" => &["a", "b", "c"],
        }
        .unwrap();

        assert!(validate_dataframe(&schema, &df).is_ok());
    }

    #[test]
    fn missing_column_detected() {
        let schema = make_schema(vec![("id", DataType::Int64), ("name", DataType::String)]);
        let df = df! { "id" => &[1i64, 2, 3] }.unwrap();

        let err = validate_dataframe(&schema, &df).unwrap_err();
        let SchemaError::ValidationFailed(report) = err else {
            panic!("expected ValidationFailed");
        };
        assert_eq!(report.missing_columns, vec!["name"]);
    }

    #[test]
    fn extra_column_detected() {
        let schema = make_schema(vec![("id", DataType::Int64)]);
        let df = df! {
            "id" => &[1i64, 2, 3],
            "extra" => &["a", "b", "c"],
        }
        .unwrap();

        let err = validate_dataframe(&schema, &df).unwrap_err();
        let SchemaError::ValidationFailed(report) = err else {
            panic!("expected ValidationFailed");
        };
        assert_eq!(report.extra_columns, vec!["extra"]);
    }

    #[test]
    fn type_mismatch_detected() {
        let schema = make_schema(vec![("id", DataType::String)]);
        let df = df! { "id" => &[1i64, 2, 3] }.unwrap();

        let err = validate_dataframe(&schema, &df).unwrap_err();
        let SchemaError::ValidationFailed(report) = err else {
            panic!("expected ValidationFailed");
        };
        assert_eq!(report.type_mismatches.len(), 1);
        assert_eq!(report.type_mismatches[0].column, "id");
        assert_eq!(report.type_mismatches[0].expected, DataType::String);
        assert_eq!(report.type_mismatches[0].actual, DataType::Int64);
    }

    #[test]
    fn order_mismatch_detected() {
        let schema = make_schema(vec![("name", DataType::String), ("id", DataType::Int64)]);
        // DataFrame has columns in opposite order
        let df = df! {
            "id" => &[1i64, 2, 3],
            "name" => &["a", "b", "c"],
        }
        .unwrap();

        let err = validate_dataframe(&schema, &df).unwrap_err();
        let SchemaError::ValidationFailed(report) = err else {
            panic!("expected ValidationFailed");
        };
        assert_eq!(report.order_mismatches.len(), 2);
    }

    #[test]
    fn multiple_errors_accumulated() {
        let schema = make_schema(vec![
            ("id", DataType::String),   // will be type mismatch (Int64 vs String)
            ("name", DataType::String), // will be missing
            ("age", DataType::Int64),   // will be missing
        ]);
        let df = df! {
            "id" => &[1i64, 2, 3],
            "extra" => &[true, false, true],
        }
        .unwrap();

        let err = validate_dataframe(&schema, &df).unwrap_err();
        let SchemaError::ValidationFailed(report) = err else {
            panic!("expected ValidationFailed");
        };
        assert_eq!(report.type_mismatches.len(), 1);
        assert_eq!(report.missing_columns.len(), 2);
        assert_eq!(report.extra_columns.len(), 1);
    }

    // ── Extra column policy tests ─────────────────────────────────

    #[test]
    fn allow_policy_tracks_extras_as_info() {
        let mut schema = make_schema(vec![("id", DataType::Int64)]);
        schema.extra_column_policy = crate::ExtraColumnPolicy::Allow;

        let df = df! {
            "id" => &[1i64, 2],
            "bonus_col" => &["a", "b"],
        }
        .unwrap();

        let report = validate_dataframe(&schema, &df).unwrap();
        assert!(report.extra_columns.is_empty());
        assert_eq!(report.info_extra_columns, vec!["bonus_col"]);
    }

    #[test]
    fn allow_policy_skips_order_mismatches() {
        let mut schema = make_schema(vec![("name", DataType::String), ("id", DataType::Int64)]);
        schema.extra_column_policy = crate::ExtraColumnPolicy::Allow;

        // Columns in opposite order
        let df = df! {
            "id" => &[1i64, 2],
            "name" => &["a", "b"],
        }
        .unwrap();

        let report = validate_dataframe(&schema, &df).unwrap();
        assert!(report.order_mismatches.is_empty());
    }

    #[test]
    fn reject_policy_still_rejects_extras() {
        let schema = make_schema(vec![("id", DataType::Int64)]);
        // default is Reject

        let df = df! {
            "id" => &[1i64],
            "extra" => &["x"],
        }
        .unwrap();

        let err = validate_dataframe(&schema, &df).unwrap_err();
        let SchemaError::ValidationFailed(report) = err else {
            panic!("expected ValidationFailed");
        };
        assert_eq!(report.extra_columns, vec!["extra"]);
        assert!(report.info_extra_columns.is_empty());
    }

    #[test]
    fn valid_df_returns_ok_report() {
        let schema = make_schema(vec![("id", DataType::Int64)]);
        let df = df! { "id" => &[1i64] }.unwrap();

        let report = validate_dataframe(&schema, &df).unwrap();
        assert!(report.is_valid());
        assert!(report.info_extra_columns.is_empty());
    }
}
