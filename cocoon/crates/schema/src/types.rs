use std::collections::HashMap;

use polars::prelude::*;
use serde::{Deserialize, Serialize};

use super::dialect::{SqlDialect, dtype_to_sql};
use super::validation::ValidationReport;

// ── Error ──────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("unknown type: {0}")]
    UnknownType(String),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("json error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("missing fields for nested type: {0}")]
    MissingFields(String),

    #[error("schema not found: {0}")]
    NotFound(String),

    #[error("validation failed:\n{0}")]
    ValidationFailed(ValidationReport),

    #[error("incompatible schema: {0}")]
    Incompatible(String),

    #[error("invalid DV2.0 schema: {0}")]
    InvalidDvSchema(String),

    #[error("data vault enrichment failed: {0}")]
    DvEnrichment(String),
}

// ── Ingestion pattern ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionPattern {
    Batch,
    Streaming,
    Cdc,
    MicroBatch,
}

// ── Schema compatibility mode ──────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CompatibilityMode {
    Backward,
    Forward,
    Full,
    #[default]
    None,
}

// ── Data Vault 2.0 table type ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DvTableType {
    Hub,
    Link,
    Satellite,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ExtraColumnPolicy {
    #[default]
    Reject,
    Allow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DvMetadata {
    Hub { business_keys: Vec<String> },
    Link { linked_hubs: Vec<String> },
    Satellite { parent: String },
}

// ── Data Vault key declarations ────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub key: String,
    pub references: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyDeclarations {
    #[serde(default)]
    pub business_key: Vec<String>,
    #[serde(default)]
    pub relationships: HashMap<String, Relationship>,
}

// ── Column constraints (Landing Zone quality checks) ───────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnConstraints {
    #[serde(default)]
    pub not_null: bool,
    #[serde(default)]
    pub unique: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

// ── Public schema types ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub version: u32,
    pub description: Option<String>,
    pub source: Option<String>,
    pub pattern: Option<IngestionPattern>,
    pub compatibility: CompatibilityMode,
    pub keys: Option<KeyDeclarations>,
    pub columns: Vec<ColumnDef>,
    pub dv_metadata: Option<DvMetadata>,
    pub extra_column_policy: ExtraColumnPolicy,
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub dtype: DataType,
    pub constraints: Option<ColumnConstraints>,
}

impl TableSchema {
    pub fn to_polars_schema(&self) -> Schema {
        Schema::from_iter(self.columns.iter().map(|c| c.to_polars_field()))
    }

    /// The declared business key columns, if any (and non-empty).
    ///
    /// A schema that declares a business key opts into Data Vault raw-vault
    /// enrichment: its uploads gain a hash key, a hashdiff and load metadata.
    pub fn business_key(&self) -> Option<&[String]> {
        self.keys
            .as_ref()
            .map(|k| k.business_key.as_slice())
            .filter(|bk| !bk.is_empty())
    }

    /// Whether this schema opts into Data Vault enrichment (see [`business_key`]).
    ///
    /// [`business_key`]: TableSchema::business_key
    pub fn is_dv_enriched(&self) -> bool {
        self.business_key().is_some()
    }
}

impl ColumnDef {
    pub fn to_polars_field(&self) -> Field {
        Field::new(self.name.as_str().into(), self.dtype.clone())
    }

    pub fn to_sql_column(&self, dialect: SqlDialect) -> String {
        format!("{} {}", self.name, dtype_to_sql(&self.dtype, dialect))
    }
}

// ── Type parser ────────────────────────────────────────────────────

/// Parse a human-friendly type string into a Polars `DataType`.
///
/// Supports all primitive types by their Polars variant name and
/// parameterized temporals like `"Datetime(us)"` and `"Duration(ms)"`.
pub fn parse_polars_dtype(s: &str) -> Result<DataType, SchemaError> {
    // Handle parameterized types: Datetime(unit) and Duration(unit)
    if let Some(inner) = s
        .strip_prefix("Datetime(")
        .and_then(|r| r.strip_suffix(')'))
    {
        let unit = parse_time_unit(inner)?;
        return Ok(DataType::Datetime(unit, None));
    }
    if let Some(inner) = s
        .strip_prefix("Duration(")
        .and_then(|r| r.strip_suffix(')'))
    {
        let unit = parse_time_unit(inner)?;
        return Ok(DataType::Duration(unit));
    }

    match s {
        "Boolean" => Ok(DataType::Boolean),
        "UInt8" => Ok(DataType::UInt8),
        "UInt16" => Ok(DataType::UInt16),
        "UInt32" => Ok(DataType::UInt32),
        "UInt64" => Ok(DataType::UInt64),
        "Int8" => Ok(DataType::Int8),
        "Int16" => Ok(DataType::Int16),
        "Int32" => Ok(DataType::Int32),
        "Int64" => Ok(DataType::Int64),
        "Float32" => Ok(DataType::Float32),
        "Float64" => Ok(DataType::Float64),
        "String" => Ok(DataType::String),
        "Binary" => Ok(DataType::Binary),
        "Date" => Ok(DataType::Date),
        "Time" => Ok(DataType::Time),
        "Null" => Ok(DataType::Null),
        _ => Err(SchemaError::UnknownType(s.to_string())),
    }
}

fn parse_time_unit(s: &str) -> Result<TimeUnit, SchemaError> {
    match s {
        "ns" => Ok(TimeUnit::Nanoseconds),
        "us" => Ok(TimeUnit::Microseconds),
        "ms" => Ok(TimeUnit::Milliseconds),
        _ => Err(SchemaError::UnknownType(format!("unknown time unit: {s}"))),
    }
}

// ── Deserialization: raw types → resolution → Deserialize impl ─────

#[derive(Deserialize)]
struct RawTableSchema {
    name: String,
    #[serde(default = "default_version")]
    version: u32,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    pattern: Option<IngestionPattern>,
    #[serde(default)]
    compatibility: CompatibilityMode,
    #[serde(default)]
    keys: Option<KeyDeclarations>,
    #[serde(default)]
    columns: Vec<RawColumnDef>,
    // DV2.0 fields
    #[serde(default)]
    table_type: Option<DvTableType>,
    #[serde(default)]
    business_keys: Option<Vec<String>>,
    #[serde(default)]
    linked_hubs: Option<Vec<String>>,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    core_columns: Option<Vec<RawColumnDef>>,
}

fn default_version() -> u32 {
    1
}

#[derive(Deserialize)]
struct RawColumnDef {
    name: String,
    #[serde(rename = "type")]
    type_str: String,
    fields: Option<Vec<RawColumnDef>>,
    #[serde(default)]
    constraints: Option<ColumnConstraints>,
}

fn resolve_column(raw: RawColumnDef) -> Result<ColumnDef, SchemaError> {
    let dtype = resolve_column_dtype(&raw.type_str, raw.fields)?;
    Ok(ColumnDef {
        name: raw.name,
        dtype,
        constraints: raw.constraints,
    })
}

fn resolve_column_dtype(
    type_str: &str,
    fields: Option<Vec<RawColumnDef>>,
) -> Result<DataType, SchemaError> {
    match type_str {
        "Struct" => {
            let raw_fields =
                fields.ok_or_else(|| SchemaError::MissingFields("Struct".to_string()))?;
            let resolved = resolve_struct_fields(raw_fields)?;
            Ok(DataType::Struct(resolved))
        }
        "List(Struct)" => {
            let raw_fields =
                fields.ok_or_else(|| SchemaError::MissingFields("List(Struct)".to_string()))?;
            let resolved = resolve_struct_fields(raw_fields)?;
            Ok(DataType::List(Box::new(DataType::Struct(resolved))))
        }
        s if s.starts_with("List(") && s.ends_with(')') => {
            let inner = &s[5..s.len() - 1];
            let inner_dtype = parse_polars_dtype(inner)?;
            Ok(DataType::List(Box::new(inner_dtype)))
        }
        s => parse_polars_dtype(s),
    }
}

fn resolve_struct_fields(raw_fields: Vec<RawColumnDef>) -> Result<Vec<Field>, SchemaError> {
    raw_fields
        .into_iter()
        .map(|raw| {
            let dtype = resolve_column_dtype(&raw.type_str, raw.fields)?;
            Ok(Field::new(raw.name.into(), dtype))
        })
        .collect()
}

// ── DV2.0 scaffolding builders ─────────────────────────────────────

pub(crate) fn hash_key_column(name: &str) -> ColumnDef {
    ColumnDef {
        name: format!("{name}_hk"),
        dtype: DataType::String,
        constraints: Some(ColumnConstraints {
            not_null: true,
            unique: false,
            pattern: None,
            min: None,
            max: None,
        }),
    }
}

/// The `hashdiff` (row hash) column — a per-row content fingerprint of the
/// descriptive columns, used to detect whether a re-loaded business key carries
/// changed data.
pub(crate) fn hashdiff_column() -> ColumnDef {
    ColumnDef {
        name: "hashdiff".to_string(),
        dtype: DataType::String,
        constraints: Some(ColumnConstraints {
            not_null: true,
            unique: false,
            pattern: None,
            min: None,
            max: None,
        }),
    }
}

/// The `load_id` column — a unique id per upload batch, tying every row of one
/// load together for traceability.
pub(crate) fn load_id_column() -> ColumnDef {
    ColumnDef {
        name: "load_id".to_string(),
        dtype: DataType::String,
        constraints: Some(ColumnConstraints {
            not_null: true,
            unique: false,
            pattern: None,
            min: None,
            max: None,
        }),
    }
}

pub(crate) fn load_date_column() -> ColumnDef {
    ColumnDef {
        name: "load_date".to_string(),
        dtype: DataType::Datetime(TimeUnit::Microseconds, None),
        constraints: Some(ColumnConstraints {
            not_null: true,
            unique: false,
            pattern: None,
            min: None,
            max: None,
        }),
    }
}

fn load_end_date_column() -> ColumnDef {
    ColumnDef {
        name: "load_end_date".to_string(),
        dtype: DataType::Datetime(TimeUnit::Microseconds, None),
        constraints: None, // nullable for active records
    }
}

pub(crate) fn record_source_column() -> ColumnDef {
    ColumnDef {
        name: "record_source".to_string(),
        dtype: DataType::String,
        constraints: Some(ColumnConstraints {
            not_null: true,
            unique: false,
            pattern: None,
            min: None,
            max: None,
        }),
    }
}

fn build_hub_scaffolding(
    name: &str,
    business_keys: &[String],
    user_columns: Vec<ColumnDef>,
) -> Result<Vec<ColumnDef>, SchemaError> {
    // Validate business keys reference declared columns
    let col_names: Vec<&str> = user_columns.iter().map(|c| c.name.as_str()).collect();
    for bk in business_keys {
        if !col_names.contains(&bk.as_str()) {
            return Err(SchemaError::InvalidDvSchema(format!(
                "business key '{bk}' not found in declared columns"
            )));
        }
    }

    let mut cols = vec![hash_key_column(name)];
    cols.extend(user_columns);
    cols.push(load_date_column());
    cols.push(record_source_column());
    Ok(cols)
}

fn build_link_scaffolding(
    name: &str,
    linked_hubs: &[String],
) -> Result<Vec<ColumnDef>, SchemaError> {
    if linked_hubs.len() < 2 {
        return Err(SchemaError::InvalidDvSchema(
            "link must reference at least 2 hubs".to_string(),
        ));
    }

    let mut cols = vec![hash_key_column(name)];
    for hub in linked_hubs {
        cols.push(hash_key_column(hub));
    }
    cols.push(load_date_column());
    cols.push(record_source_column());
    Ok(cols)
}

fn build_satellite_scaffolding(parent: &str, core_columns: Vec<ColumnDef>) -> Vec<ColumnDef> {
    let mut cols = vec![hash_key_column(parent)];
    cols.extend(core_columns);
    cols.push(load_date_column());
    cols.push(load_end_date_column());
    cols.push(record_source_column());
    cols
}

// ── Schema resolution (legacy + DV2.0) ────────────────────────────

fn resolve_table_schema(raw: RawTableSchema) -> Result<TableSchema, SchemaError> {
    match raw.table_type {
        None => {
            // Legacy path — unchanged
            let columns: Vec<ColumnDef> = raw
                .columns
                .into_iter()
                .map(resolve_column)
                .collect::<Result<_, _>>()?;
            Ok(TableSchema {
                name: raw.name,
                version: raw.version,
                description: raw.description,
                source: raw.source,
                pattern: raw.pattern,
                compatibility: raw.compatibility,
                keys: raw.keys,
                columns,
                dv_metadata: None,
                extra_column_policy: ExtraColumnPolicy::default(),
            })
        }
        Some(DvTableType::Hub) => {
            let business_keys = raw.business_keys.ok_or_else(|| {
                SchemaError::InvalidDvSchema("hub schema requires 'business_keys'".to_string())
            })?;
            if business_keys.is_empty() {
                return Err(SchemaError::InvalidDvSchema(
                    "hub schema requires at least one business key".to_string(),
                ));
            }
            let user_columns: Vec<ColumnDef> = raw
                .columns
                .into_iter()
                .map(resolve_column)
                .collect::<Result<_, _>>()?;
            let columns = build_hub_scaffolding(&raw.name, &business_keys, user_columns)?;
            Ok(TableSchema {
                name: raw.name,
                version: raw.version,
                description: raw.description,
                source: raw.source,
                pattern: raw.pattern,
                compatibility: raw.compatibility,
                keys: raw.keys,
                columns,
                dv_metadata: Some(DvMetadata::Hub { business_keys }),
                extra_column_policy: ExtraColumnPolicy::Reject,
            })
        }
        Some(DvTableType::Link) => {
            let linked_hubs = raw.linked_hubs.ok_or_else(|| {
                SchemaError::InvalidDvSchema("link schema requires 'linked_hubs'".to_string())
            })?;
            let columns = build_link_scaffolding(&raw.name, &linked_hubs)?;
            Ok(TableSchema {
                name: raw.name,
                version: raw.version,
                description: raw.description,
                source: raw.source,
                pattern: raw.pattern,
                compatibility: raw.compatibility,
                keys: raw.keys,
                columns,
                dv_metadata: Some(DvMetadata::Link { linked_hubs }),
                extra_column_policy: ExtraColumnPolicy::Reject,
            })
        }
        Some(DvTableType::Satellite) => {
            let parent = raw.parent.ok_or_else(|| {
                SchemaError::InvalidDvSchema("satellite schema requires 'parent'".to_string())
            })?;
            // Use core_columns if present, otherwise fall back to columns
            let raw_cols = raw.core_columns.unwrap_or(raw.columns);
            let core_columns: Vec<ColumnDef> = raw_cols
                .into_iter()
                .map(resolve_column)
                .collect::<Result<_, _>>()?;
            let columns = build_satellite_scaffolding(&parent, core_columns);
            Ok(TableSchema {
                name: raw.name,
                version: raw.version,
                description: raw.description,
                source: raw.source,
                pattern: raw.pattern,
                compatibility: raw.compatibility,
                keys: raw.keys,
                columns,
                dv_metadata: Some(DvMetadata::Satellite { parent }),
                extra_column_policy: ExtraColumnPolicy::Allow,
            })
        }
    }
}

impl<'de> Deserialize<'de> for TableSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawTableSchema::deserialize(deserializer)?;
        resolve_table_schema(raw).map_err(serde::de::Error::custom)
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_polars_dtype ──────────────────────────────────────

    #[test]
    fn parse_all_primitive_types() {
        let cases = vec![
            ("Boolean", DataType::Boolean),
            ("UInt8", DataType::UInt8),
            ("UInt16", DataType::UInt16),
            ("UInt32", DataType::UInt32),
            ("UInt64", DataType::UInt64),
            ("Int8", DataType::Int8),
            ("Int16", DataType::Int16),
            ("Int32", DataType::Int32),
            ("Int64", DataType::Int64),
            ("Float32", DataType::Float32),
            ("Float64", DataType::Float64),
            ("String", DataType::String),
            ("Binary", DataType::Binary),
            ("Date", DataType::Date),
            ("Time", DataType::Time),
            ("Null", DataType::Null),
        ];
        for (input, expected) in cases {
            assert_eq!(
                parse_polars_dtype(input).unwrap(),
                expected,
                "failed for {input}"
            );
        }
    }

    #[test]
    fn parse_parameterized_temporals() {
        assert_eq!(
            parse_polars_dtype("Datetime(us)").unwrap(),
            DataType::Datetime(TimeUnit::Microseconds, None)
        );
        assert_eq!(
            parse_polars_dtype("Datetime(ms)").unwrap(),
            DataType::Datetime(TimeUnit::Milliseconds, None)
        );
        assert_eq!(
            parse_polars_dtype("Datetime(ns)").unwrap(),
            DataType::Datetime(TimeUnit::Nanoseconds, None)
        );
        assert_eq!(
            parse_polars_dtype("Duration(ms)").unwrap(),
            DataType::Duration(TimeUnit::Milliseconds)
        );
    }

    #[test]
    fn parse_unknown_type_rejected() {
        let err = parse_polars_dtype("Varchar").unwrap_err();
        assert!(matches!(err, SchemaError::UnknownType(_)));
    }

    // ── JSON deserialization ────────────────────────────────────

    #[test]
    fn deserialize_schema_from_json() {
        let json = r#"{
            "name": "users",
            "columns": [
                { "name": "id", "type": "Int64" },
                { "name": "name", "type": "String" },
                { "name": "score", "type": "Float64" },
                { "name": "active", "type": "Boolean" }
            ]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.name, "users");
        assert_eq!(schema.columns.len(), 4);
        assert_eq!(schema.columns[0].dtype, DataType::Int64);
        assert_eq!(schema.columns[1].dtype, DataType::String);
        assert_eq!(schema.columns[2].dtype, DataType::Float64);
        assert_eq!(schema.columns[3].dtype, DataType::Boolean);
    }

    #[test]
    fn deserialize_schema_with_temporals() {
        let json = r#"{
            "name": "events",
            "columns": [
                { "name": "ts", "type": "Datetime(us)" },
                { "name": "dur", "type": "Duration(ms)" }
            ]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(
            schema.columns[0].dtype,
            DataType::Datetime(TimeUnit::Microseconds, None)
        );
        assert_eq!(
            schema.columns[1].dtype,
            DataType::Duration(TimeUnit::Milliseconds)
        );
    }

    #[test]
    fn deserialize_unknown_type_fails() {
        let json = r#"{
            "name": "bad",
            "columns": [
                { "name": "x", "type": "Varchar" }
            ]
        }"#;

        let result: Result<TableSchema, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // ── Nested schema deserialization ──────────────────────────

    #[test]
    fn deserialize_struct_column() {
        let json = r#"{
            "name": "orders",
            "columns": [
                { "name": "id", "type": "Int64" },
                { "name": "customer", "type": "Struct", "fields": [
                    { "name": "name", "type": "String" },
                    { "name": "age", "type": "Int32" }
                ]}
            ]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.columns.len(), 2);
        assert_eq!(
            schema.columns[1].dtype,
            DataType::Struct(vec![
                Field::new("name".into(), DataType::String),
                Field::new("age".into(), DataType::Int32),
            ])
        );
    }

    #[test]
    fn deserialize_list_of_primitives() {
        let json = r#"{
            "name": "tags_table",
            "columns": [
                { "name": "tags", "type": "List(String)" },
                { "name": "scores", "type": "List(Int64)" }
            ]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(
            schema.columns[0].dtype,
            DataType::List(Box::new(DataType::String))
        );
        assert_eq!(
            schema.columns[1].dtype,
            DataType::List(Box::new(DataType::Int64))
        );
    }

    #[test]
    fn deserialize_list_of_struct() {
        let json = r#"{
            "name": "orders",
            "columns": [
                { "name": "line_items", "type": "List(Struct)", "fields": [
                    { "name": "sku", "type": "String" },
                    { "name": "qty", "type": "Int32" }
                ]}
            ]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(
            schema.columns[0].dtype,
            DataType::List(Box::new(DataType::Struct(vec![
                Field::new("sku".into(), DataType::String),
                Field::new("qty".into(), DataType::Int32),
            ])))
        );
    }

    #[test]
    fn deserialize_nested_struct() {
        let json = r#"{
            "name": "deep",
            "columns": [
                { "name": "customer", "type": "Struct", "fields": [
                    { "name": "name", "type": "String" },
                    { "name": "address", "type": "Struct", "fields": [
                        { "name": "street", "type": "String" },
                        { "name": "city", "type": "String" }
                    ]}
                ]}
            ]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(
            schema.columns[0].dtype,
            DataType::Struct(vec![
                Field::new("name".into(), DataType::String),
                Field::new(
                    "address".into(),
                    DataType::Struct(vec![
                        Field::new("street".into(), DataType::String),
                        Field::new("city".into(), DataType::String),
                    ])
                ),
            ])
        );
    }

    #[test]
    fn deserialize_struct_missing_fields_fails() {
        let json = r#"{
            "name": "bad",
            "columns": [
                { "name": "data", "type": "Struct" }
            ]
        }"#;

        let result: Result<TableSchema, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("missing fields"), "got: {err_msg}");
    }

    #[test]
    fn deserialize_list_struct_missing_fields_fails() {
        let json = r#"{
            "name": "bad",
            "columns": [
                { "name": "items", "type": "List(Struct)" }
            ]
        }"#;

        let result: Result<TableSchema, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("missing fields"), "got: {err_msg}");
    }

    #[test]
    fn deserialize_list_of_temporal() {
        let json = r#"{
            "name": "temporal",
            "columns": [
                { "name": "timestamps", "type": "List(Datetime(us))" }
            ]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(
            schema.columns[0].dtype,
            DataType::List(Box::new(DataType::Datetime(TimeUnit::Microseconds, None)))
        );
    }

    // ── to_polars_schema ───────────────────────────────────────

    fn make_schema(cols: Vec<(&str, DataType)>) -> TableSchema {
        TableSchema {
            name: "test".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
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
            extra_column_policy: ExtraColumnPolicy::default(),
        }
    }

    #[test]
    fn to_polars_schema_flat() {
        let schema = make_schema(vec![("id", DataType::Int64), ("name", DataType::String)]);
        let polars_schema = schema.to_polars_schema();

        assert_eq!(polars_schema.len(), 2);
        assert_eq!(polars_schema.get("id").unwrap(), &DataType::Int64);
        assert_eq!(polars_schema.get("name").unwrap(), &DataType::String);
    }

    #[test]
    fn to_polars_schema_nested() {
        let nested_dtype = DataType::Struct(vec![
            Field::new("street".into(), DataType::String),
            Field::new("city".into(), DataType::String),
        ]);
        let schema = make_schema(vec![
            ("id", DataType::Int64),
            ("address", nested_dtype.clone()),
        ]);
        let polars_schema = schema.to_polars_schema();

        assert_eq!(polars_schema.len(), 2);
        assert_eq!(polars_schema.get("address").unwrap(), &nested_dtype);
    }

    // ── Metadata deserialization ──────────────────────────────────

    #[test]
    fn deserialize_minimal_schema_defaults() {
        let json = r#"{
            "name": "bare",
            "columns": [{ "name": "id", "type": "Int64" }]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.version, 1);
        assert!(schema.description.is_none());
        assert!(schema.source.is_none());
        assert!(schema.pattern.is_none());
        assert_eq!(schema.compatibility, CompatibilityMode::None);
        assert!(schema.keys.is_none());
    }

    #[test]
    fn deserialize_full_metadata() {
        let json = r#"{
            "name": "orders",
            "version": 3,
            "description": "Customer orders from ERP",
            "source": "sap_erp",
            "pattern": "batch",
            "compatibility": "BACKWARD",
            "keys": {
                "business_key": ["order_id"],
                "relationships": {
                    "customer": { "key": "customer_id", "references": "customers" }
                }
            },
            "columns": [
                { "name": "order_id", "type": "String" },
                { "name": "customer_id", "type": "String" },
                { "name": "total", "type": "Float64" }
            ]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.version, 3);
        assert_eq!(
            schema.description.as_deref(),
            Some("Customer orders from ERP")
        );
        assert_eq!(schema.source.as_deref(), Some("sap_erp"));
        assert_eq!(schema.pattern, Some(IngestionPattern::Batch));
        assert_eq!(schema.compatibility, CompatibilityMode::Backward);

        let keys = schema.keys.unwrap();
        assert_eq!(keys.business_key, vec!["order_id"]);
        let rel = keys.relationships.get("customer").unwrap();
        assert_eq!(rel.key, "customer_id");
        assert_eq!(rel.references, "customers");
    }

    #[test]
    fn deserialize_column_with_constraints() {
        let json = r#"{
            "name": "constrained",
            "columns": [{
                "name": "customer_id",
                "type": "String",
                "constraints": {
                    "not_null": true,
                    "unique": true,
                    "pattern": "^CUS-[0-9]{6}$"
                }
            }]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        let c = schema.columns[0].constraints.as_ref().unwrap();
        assert!(c.not_null);
        assert!(c.unique);
        assert_eq!(c.pattern.as_deref(), Some("^CUS-[0-9]{6}$"));
        assert!(c.min.is_none());
    }

    #[test]
    fn deserialize_all_ingestion_patterns() {
        for (json_val, expected) in [
            ("batch", IngestionPattern::Batch),
            ("streaming", IngestionPattern::Streaming),
            ("cdc", IngestionPattern::Cdc),
            ("micro_batch", IngestionPattern::MicroBatch),
        ] {
            let json = format!(
                r#"{{"name":"t","pattern":"{}","columns":[{{"name":"x","type":"Int64"}}]}}"#,
                json_val
            );
            let schema: TableSchema = serde_json::from_str(&json).unwrap();
            assert_eq!(schema.pattern, Some(expected), "failed for {json_val}");
        }
    }

    // ── Data Vault 2.0 deserialization ────────────────────────────

    #[test]
    fn deserialize_hub_schema() {
        let json = r#"{
            "name": "hub_customer",
            "table_type": "hub",
            "business_keys": ["customer_id"],
            "columns": [
                { "name": "customer_id", "type": "String", "constraints": { "not_null": true } }
            ]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.name, "hub_customer");

        // Resolved columns: [hub_customer_hk, customer_id, load_date, record_source]
        assert_eq!(schema.columns.len(), 4);
        assert_eq!(schema.columns[0].name, "hub_customer_hk");
        assert_eq!(schema.columns[0].dtype, DataType::String);
        assert_eq!(schema.columns[1].name, "customer_id");
        assert_eq!(schema.columns[1].dtype, DataType::String);
        assert_eq!(schema.columns[2].name, "load_date");
        assert_eq!(
            schema.columns[2].dtype,
            DataType::Datetime(TimeUnit::Microseconds, None)
        );
        assert_eq!(schema.columns[3].name, "record_source");
        assert_eq!(schema.columns[3].dtype, DataType::String);

        assert_eq!(
            schema.dv_metadata,
            Some(DvMetadata::Hub {
                business_keys: vec!["customer_id".to_string()]
            })
        );
        assert_eq!(schema.extra_column_policy, ExtraColumnPolicy::Reject);
    }

    #[test]
    fn deserialize_link_schema() {
        let json = r#"{
            "name": "lnk_customer_order",
            "table_type": "link",
            "linked_hubs": ["hub_customer", "hub_order"]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.name, "lnk_customer_order");

        // Resolved columns: [lnk_customer_order_hk, hub_customer_hk, hub_order_hk, load_date, record_source]
        assert_eq!(schema.columns.len(), 5);
        assert_eq!(schema.columns[0].name, "lnk_customer_order_hk");
        assert_eq!(schema.columns[1].name, "hub_customer_hk");
        assert_eq!(schema.columns[2].name, "hub_order_hk");
        assert_eq!(schema.columns[3].name, "load_date");
        assert_eq!(schema.columns[4].name, "record_source");

        assert_eq!(
            schema.dv_metadata,
            Some(DvMetadata::Link {
                linked_hubs: vec!["hub_customer".to_string(), "hub_order".to_string()]
            })
        );
        assert_eq!(schema.extra_column_policy, ExtraColumnPolicy::Reject);
    }

    #[test]
    fn deserialize_satellite_schema() {
        let json = r#"{
            "name": "sat_customer_demographics",
            "table_type": "satellite",
            "parent": "hub_customer",
            "core_columns": [
                { "name": "email", "type": "String" }
            ]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.name, "sat_customer_demographics");

        // Resolved columns: [hub_customer_hk, email, load_date, load_end_date, record_source]
        assert_eq!(schema.columns.len(), 5);
        assert_eq!(schema.columns[0].name, "hub_customer_hk");
        assert_eq!(schema.columns[1].name, "email");
        assert_eq!(schema.columns[2].name, "load_date");
        assert_eq!(schema.columns[3].name, "load_end_date");
        assert_eq!(schema.columns[4].name, "record_source");

        // load_end_date should be nullable (no constraints)
        assert!(schema.columns[3].constraints.is_none());

        assert_eq!(
            schema.dv_metadata,
            Some(DvMetadata::Satellite {
                parent: "hub_customer".to_string()
            })
        );
        assert_eq!(schema.extra_column_policy, ExtraColumnPolicy::Allow);
    }

    #[test]
    fn hub_missing_business_keys_fails() {
        let json = r#"{
            "name": "hub_bad",
            "table_type": "hub",
            "columns": [{ "name": "id", "type": "String" }]
        }"#;

        let result: Result<TableSchema, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("business_keys"), "got: {msg}");
    }

    #[test]
    fn hub_business_key_not_in_columns_fails() {
        let json = r#"{
            "name": "hub_bad",
            "table_type": "hub",
            "business_keys": ["missing_col"],
            "columns": [{ "name": "id", "type": "String" }]
        }"#;

        let result: Result<TableSchema, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("missing_col"), "got: {msg}");
    }

    #[test]
    fn link_fewer_than_two_hubs_fails() {
        let json = r#"{
            "name": "lnk_bad",
            "table_type": "link",
            "linked_hubs": ["hub_only_one"]
        }"#;

        let result: Result<TableSchema, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("at least 2"), "got: {msg}");
    }

    #[test]
    fn satellite_missing_parent_fails() {
        let json = r#"{
            "name": "sat_bad",
            "table_type": "satellite",
            "core_columns": [{ "name": "x", "type": "String" }]
        }"#;

        let result: Result<TableSchema, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("parent"), "got: {msg}");
    }

    #[test]
    fn legacy_schema_no_table_type_unchanged() {
        let json = r#"{
            "name": "legacy",
            "columns": [{ "name": "id", "type": "Int64" }]
        }"#;

        let schema: TableSchema = serde_json::from_str(json).unwrap();
        assert!(schema.dv_metadata.is_none());
        assert_eq!(schema.extra_column_policy, ExtraColumnPolicy::Reject);
        assert_eq!(schema.columns.len(), 1);
        assert_eq!(schema.columns[0].name, "id");
    }
}
