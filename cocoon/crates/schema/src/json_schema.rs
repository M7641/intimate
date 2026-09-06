use polars::prelude::*;
use serde_json::{Value, json};

use super::types::TableSchema;

// ── JSON Schema generation ───────────────────────────────────────

/// Convert a `TableSchema` into a JSON Schema document (draft 2020-12).
///
/// Produces a top-level `"object"` schema with `"properties"` derived
/// from each `ColumnDef`. Columns with `not_null` constraint are added
/// to the `"required"` array.
pub fn to_json_schema(schema: &TableSchema) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for col in &schema.columns {
        let col_schema = dtype_to_json_schema(&col.dtype);

        // Apply constraint annotations
        let col_schema = if let Some(constraints) = &col.constraints {
            let mut s = col_schema;
            if constraints.not_null {
                required.push(Value::String(col.name.clone()));
            }
            if let Some(pat) = &constraints.pattern {
                s.as_object_mut()
                    .map(|o| o.insert("pattern".to_string(), json!(pat)));
            }
            if let Some(min) = constraints.min {
                s.as_object_mut()
                    .map(|o| o.insert("minimum".to_string(), json!(min)));
            }
            if let Some(max) = constraints.max {
                s.as_object_mut()
                    .map(|o| o.insert("maximum".to_string(), json!(max)));
            }
            s
        } else {
            col_schema
        };

        properties.insert(col.name.clone(), col_schema);
    }

    let mut schema_obj = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": schema.name,
        "type": "object",
        "properties": Value::Object(properties),
    });

    if !required.is_empty() {
        schema_obj
            .as_object_mut()
            .unwrap()
            .insert("required".to_string(), Value::Array(required));
    }

    schema_obj
}

/// Map a Polars `DataType` to its JSON Schema representation.
pub fn dtype_to_json_schema(dtype: &DataType) -> Value {
    match dtype {
        DataType::Boolean => json!({"type": "boolean"}),

        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => {
            json!({"type": "integer"})
        }

        DataType::Float32 | DataType::Float64 => json!({"type": "number"}),

        DataType::String => json!({"type": "string"}),

        DataType::Binary => json!({
            "type": "string",
            "contentEncoding": "base64",
        }),

        DataType::Date => json!({
            "type": "string",
            "format": "date",
        }),

        DataType::Time => json!({
            "type": "string",
            "format": "time",
        }),

        DataType::Datetime(_, _) => json!({
            "type": "string",
            "format": "date-time",
        }),

        DataType::Duration(_) => json!({
            "type": "string",
            "description": "ISO 8601 duration",
        }),

        DataType::Null => json!({"type": "null"}),

        DataType::List(inner) => json!({
            "type": "array",
            "items": dtype_to_json_schema(inner),
        }),

        DataType::Struct(fields) => {
            let mut props = serde_json::Map::new();
            for f in fields {
                props.insert(f.name().to_string(), dtype_to_json_schema(f.dtype()));
            }
            json!({
                "type": "object",
                "properties": Value::Object(props),
            })
        }

        _ => json!({"type": "string"}), // fallback
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColumnConstraints, ColumnDef, CompatibilityMode, ExtraColumnPolicy};

    fn make_schema(name: &str, cols: Vec<(&str, DataType)>) -> TableSchema {
        TableSchema {
            name: name.to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: cols
                .into_iter()
                .map(|(n, dtype)| ColumnDef {
                    name: n.to_string(),
                    dtype,
                    constraints: None,
                })
                .collect(),
            dv_metadata: None,
            extra_column_policy: ExtraColumnPolicy::default(),
        }
    }

    #[test]
    fn json_schema_primitives() {
        assert_eq!(
            dtype_to_json_schema(&DataType::Boolean),
            json!({"type": "boolean"})
        );
        assert_eq!(
            dtype_to_json_schema(&DataType::Int64),
            json!({"type": "integer"})
        );
        assert_eq!(
            dtype_to_json_schema(&DataType::Float64),
            json!({"type": "number"})
        );
        assert_eq!(
            dtype_to_json_schema(&DataType::String),
            json!({"type": "string"})
        );
    }

    #[test]
    fn json_schema_temporals() {
        assert_eq!(
            dtype_to_json_schema(&DataType::Date),
            json!({"type": "string", "format": "date"})
        );
        assert_eq!(
            dtype_to_json_schema(&DataType::Datetime(TimeUnit::Microseconds, None)),
            json!({"type": "string", "format": "date-time"})
        );
        assert_eq!(
            dtype_to_json_schema(&DataType::Time),
            json!({"type": "string", "format": "time"})
        );
    }

    #[test]
    fn json_schema_binary() {
        assert_eq!(
            dtype_to_json_schema(&DataType::Binary),
            json!({"type": "string", "contentEncoding": "base64"})
        );
    }

    #[test]
    fn json_schema_list() {
        let result = dtype_to_json_schema(&DataType::List(Box::new(DataType::Int64)));
        assert_eq!(
            result,
            json!({"type": "array", "items": {"type": "integer"}})
        );
    }

    #[test]
    fn json_schema_struct() {
        let dt = DataType::Struct(vec![
            Field::new("name".into(), DataType::String),
            Field::new("age".into(), DataType::Int32),
        ]);
        let result = dtype_to_json_schema(&dt);
        assert_eq!(
            result,
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "age": {"type": "integer"},
                }
            })
        );
    }

    #[test]
    fn json_schema_full_document() {
        let schema = make_schema(
            "users",
            vec![
                ("id", DataType::Int64),
                ("name", DataType::String),
                ("created_at", DataType::Date),
            ],
        );
        let js = to_json_schema(&schema);

        assert_eq!(js["title"], "users");
        assert_eq!(js["type"], "object");
        assert_eq!(js["properties"]["id"]["type"], "integer");
        assert_eq!(js["properties"]["name"]["type"], "string");
        assert_eq!(js["properties"]["created_at"]["format"], "date");
        // No required array when no not_null constraints
        assert!(js.get("required").is_none());
    }

    #[test]
    fn json_schema_with_constraints() {
        let schema = TableSchema {
            name: "constrained".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: vec![
                ColumnDef {
                    name: "id".to_string(),
                    dtype: DataType::String,
                    constraints: Some(ColumnConstraints {
                        not_null: true,
                        unique: false,
                        pattern: Some("^ID-[0-9]+$".to_string()),
                        min: None,
                        max: None,
                    }),
                },
                ColumnDef {
                    name: "score".to_string(),
                    dtype: DataType::Float64,
                    constraints: Some(ColumnConstraints {
                        not_null: false,
                        unique: false,
                        pattern: None,
                        min: Some(0.0),
                        max: Some(100.0),
                    }),
                },
            ],
            dv_metadata: None,
            extra_column_policy: ExtraColumnPolicy::default(),
        };
        let js = to_json_schema(&schema);

        // "id" is required (not_null)
        let required = js["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "id");

        // Pattern constraint applied
        assert_eq!(js["properties"]["id"]["pattern"], "^ID-[0-9]+$");

        // Min/max constraints applied
        assert_eq!(js["properties"]["score"]["minimum"], 0.0);
        assert_eq!(js["properties"]["score"]["maximum"], 100.0);
    }
}
