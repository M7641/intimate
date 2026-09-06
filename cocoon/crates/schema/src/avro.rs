use polars::prelude::*;
use serde_json::{Value, json};

use super::types::TableSchema;

// ── Avro schema generation ───────────────────────────────────────

/// Convert a `TableSchema` into an Avro schema JSON value.
///
/// Produces a top-level `"record"` with the schema name and a `"fields"`
/// array derived from each `ColumnDef`.
pub fn to_avro_schema(schema: &TableSchema) -> Value {
    let fields: Vec<Value> = schema
        .columns
        .iter()
        .map(|col| {
            let avro_type = dtype_to_avro(&col.dtype, &col.name);
            let nullable = col.constraints.as_ref().is_none_or(|c| !c.not_null);
            json!({
                "name": col.name,
                "type": if nullable { json!(["null", avro_type]) } else { avro_type },
            })
        })
        .collect();

    json!({
        "type": "record",
        "name": schema.name,
        "fields": fields,
    })
}

/// Map a Polars `DataType` to its Avro schema representation.
///
/// Returns a `serde_json::Value` because Avro schemas range from simple
/// strings (`"long"`) to nested objects (`{"type": "record", ...}`).
pub fn dtype_to_avro(dtype: &DataType, field_name: &str) -> Value {
    match dtype {
        DataType::Boolean => json!("boolean"),

        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::UInt8 | DataType::UInt16 => {
            json!("int")
        }

        DataType::Int64 | DataType::UInt32 | DataType::UInt64 => json!("long"),

        DataType::Float32 => json!("float"),
        DataType::Float64 => json!("double"),

        DataType::String => json!("string"),
        DataType::Binary => json!("bytes"),

        DataType::Date => json!({
            "type": "int",
            "logicalType": "date",
        }),

        DataType::Time => json!({
            "type": "long",
            "logicalType": "time-micros",
        }),

        DataType::Datetime(unit, _) => {
            let logical = match unit {
                TimeUnit::Milliseconds => "timestamp-millis",
                TimeUnit::Microseconds | TimeUnit::Nanoseconds => "timestamp-micros",
            };
            json!({
                "type": "long",
                "logicalType": logical,
            })
        }

        DataType::Duration(_) => json!({
            "type": "long",
            "logicalType": "duration-micros",
        }),

        DataType::Null => json!("null"),

        DataType::List(inner) => json!({
            "type": "array",
            "items": dtype_to_avro(inner, field_name),
        }),

        DataType::Struct(fields) => {
            let avro_fields: Vec<Value> = fields
                .iter()
                .map(|f| {
                    json!({
                        "name": f.name().as_str(),
                        "type": dtype_to_avro(f.dtype(), f.name().as_str()),
                    })
                })
                .collect();
            json!({
                "type": "record",
                "name": field_name,
                "fields": avro_fields,
            })
        }

        _ => json!("string"), // fallback
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
    fn avro_primitives() {
        assert_eq!(dtype_to_avro(&DataType::Boolean, "x"), json!("boolean"));
        assert_eq!(dtype_to_avro(&DataType::Int32, "x"), json!("int"));
        assert_eq!(dtype_to_avro(&DataType::Int64, "x"), json!("long"));
        assert_eq!(dtype_to_avro(&DataType::Float32, "x"), json!("float"));
        assert_eq!(dtype_to_avro(&DataType::Float64, "x"), json!("double"));
        assert_eq!(dtype_to_avro(&DataType::String, "x"), json!("string"));
        assert_eq!(dtype_to_avro(&DataType::Binary, "x"), json!("bytes"));
    }

    #[test]
    fn avro_unsigned_promotion() {
        assert_eq!(dtype_to_avro(&DataType::UInt8, "x"), json!("int"));
        assert_eq!(dtype_to_avro(&DataType::UInt16, "x"), json!("int"));
        assert_eq!(dtype_to_avro(&DataType::UInt32, "x"), json!("long"));
        assert_eq!(dtype_to_avro(&DataType::UInt64, "x"), json!("long"));
    }

    #[test]
    fn avro_temporals() {
        assert_eq!(
            dtype_to_avro(&DataType::Date, "x"),
            json!({"type": "int", "logicalType": "date"})
        );
        assert_eq!(
            dtype_to_avro(&DataType::Time, "x"),
            json!({"type": "long", "logicalType": "time-micros"})
        );
        assert_eq!(
            dtype_to_avro(&DataType::Datetime(TimeUnit::Microseconds, None), "x"),
            json!({"type": "long", "logicalType": "timestamp-micros"})
        );
        assert_eq!(
            dtype_to_avro(&DataType::Datetime(TimeUnit::Milliseconds, None), "x"),
            json!({"type": "long", "logicalType": "timestamp-millis"})
        );
    }

    #[test]
    fn avro_list() {
        let result = dtype_to_avro(&DataType::List(Box::new(DataType::String)), "tags");
        assert_eq!(result, json!({"type": "array", "items": "string"}));
    }

    #[test]
    fn avro_struct() {
        let dt = DataType::Struct(vec![
            Field::new("name".into(), DataType::String),
            Field::new("age".into(), DataType::Int32),
        ]);
        let result = dtype_to_avro(&dt, "person");
        assert_eq!(
            result,
            json!({
                "type": "record",
                "name": "person",
                "fields": [
                    {"name": "name", "type": "string"},
                    {"name": "age", "type": "int"},
                ]
            })
        );
    }

    #[test]
    fn avro_full_schema() {
        let schema = make_schema(
            "users",
            vec![
                ("id", DataType::Int64),
                ("name", DataType::String),
                ("active", DataType::Boolean),
            ],
        );
        let avro = to_avro_schema(&schema);

        assert_eq!(avro["type"], "record");
        assert_eq!(avro["name"], "users");
        assert_eq!(avro["fields"].as_array().unwrap().len(), 3);
        // All nullable by default (no constraints)
        assert_eq!(avro["fields"][0]["type"], json!(["null", "long"]));
    }

    #[test]
    fn avro_not_null_constraint_removes_union() {
        let schema = TableSchema {
            name: "strict".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: vec![ColumnDef {
                name: "id".to_string(),
                dtype: DataType::Int64,
                constraints: Some(ColumnConstraints {
                    not_null: true,
                    unique: false,
                    pattern: None,
                    min: None,
                    max: None,
                }),
            }],
            dv_metadata: None,
            extra_column_policy: ExtraColumnPolicy::default(),
        };
        let avro = to_avro_schema(&schema);
        // Not nullable — bare type, no union
        assert_eq!(avro["fields"][0]["type"], json!("long"));
    }
}
