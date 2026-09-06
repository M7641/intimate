use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde_json::Value;
use tracing::info;

use super::avro::to_avro_schema;
use super::dialect::SqlDialect;
use super::json_schema::to_json_schema;
use super::types::{CompatibilityMode, DvMetadata, SchemaError, TableSchema};
use super::validation::validate_dataframe;

// ── Registry ──────────────────────────────────────────────────────

/// Thread-safe cache of named schemas with lazy loading from disk.
///
/// Supports both unversioned (`{key}.json`) and versioned (`{key}_v{N}.json`)
/// schema files. The latest version is always used for validation unless
/// a specific version is requested.
///
/// Uses `std::sync::RwLock` since the lock is only held for
/// HashMap operations (microseconds) and never across `.await`.
pub struct SchemaRegistry {
    schemas: RwLock<HashMap<String, TableSchema>>,
    versions: RwLock<HashMap<String, BTreeMap<u32, TableSchema>>>,
    schemas_dir: PathBuf,
}

impl SchemaRegistry {
    pub fn new(schemas_dir: impl Into<PathBuf>) -> Self {
        Self {
            schemas: RwLock::new(HashMap::new()),
            versions: RwLock::new(HashMap::new()),
            schemas_dir: schemas_dir.into(),
        }
    }

    /// Load a schema from a JSON file and cache it under `key`.
    pub fn load_from_file(&self, key: &str, path: &Path) -> Result<(), SchemaError> {
        let data = std::fs::read_to_string(path)?;
        let schema: TableSchema = serde_json::from_str(&data)?;
        self.insert(key, schema);
        Ok(())
    }

    /// Cache a pre-built schema under `key`.
    ///
    /// Also records it in the version history under its `version` field.
    pub fn insert(&self, key: &str, schema: TableSchema) {
        let version = schema.version;
        self.schemas
            .write()
            .expect("schema registry lock poisoned")
            .insert(key.to_string(), schema.clone());
        self.versions
            .write()
            .expect("version registry lock poisoned")
            .entry(key.to_string())
            .or_default()
            .insert(version, schema);
    }

    /// Retrieve a schema by key (latest version). Returns a clone.
    pub fn get(&self, key: &str) -> Option<TableSchema> {
        self.schemas
            .read()
            .expect("schema registry lock poisoned")
            .get(key)
            .cloned()
    }

    /// Retrieve a specific version of a schema.
    pub fn get_version(&self, key: &str, version: u32) -> Option<TableSchema> {
        self.versions
            .read()
            .expect("version registry lock poisoned")
            .get(key)
            .and_then(|versions| versions.get(&version).cloned())
    }

    /// List all registered versions for a key, in ascending order.
    pub fn list_versions(&self, key: &str) -> Vec<u32> {
        self.versions
            .read()
            .expect("version registry lock poisoned")
            .get(key)
            .map(|versions| versions.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Get a cached schema or load it from `{schemas_dir}/{key}.json`.
    ///
    /// On cache miss, reads the file, deserializes, caches, and returns it.
    /// Subsequent calls for the same key hit the in-memory cache.
    pub fn get_or_load(&self, key: &str) -> Result<TableSchema, SchemaError> {
        if let Some(schema) = self.get(key) {
            return Ok(schema);
        }

        let path = self.schemas_dir.join(format!("{key}.json"));
        info!(schema = %key, path = %path.display(), "Loading schema from disk");
        self.load_from_file(key, &path)?;

        self.get(key)
            .ok_or_else(|| SchemaError::NotFound(key.to_string()))
    }

    /// Register a new schema version with compatibility checking.
    ///
    /// If a previous version exists, checks compatibility according to the
    /// schema's declared `compatibility` mode. Returns an error if the new
    /// version is incompatible.
    pub fn register_version(&self, key: &str, schema: TableSchema) -> Result<(), SchemaError> {
        // Find the previous latest version
        let prev = {
            let versions = self
                .versions
                .read()
                .expect("version registry lock poisoned");
            versions.get(key).and_then(|v| v.values().last().cloned())
        };

        if let Some(prev_schema) = prev {
            check_compatibility(&prev_schema, &schema)?;
        }

        // Enforce parent reference for satellites
        if let Some(DvMetadata::Satellite { ref parent }) = schema.dv_metadata {
            let parent_schema = self.get(parent).ok_or_else(|| {
                SchemaError::InvalidDvSchema(format!(
                    "satellite '{}' references parent '{}' which is not registered",
                    key, parent
                ))
            })?;
            match parent_schema.dv_metadata {
                Some(DvMetadata::Hub { .. }) | Some(DvMetadata::Link { .. }) => {}
                _ => {
                    return Err(SchemaError::InvalidDvSchema(format!(
                        "satellite '{}' parent '{}' must be a hub or link, not a legacy/satellite schema",
                        key, parent
                    )));
                }
            }
        }

        self.insert(key, schema);
        Ok(())
    }

    // ── Format converters ─────────────────────────────────────────

    /// Generate an Avro schema JSON for the given key.
    pub fn to_avro_schema(&self, key: &str) -> Result<Value, SchemaError> {
        let schema = self.get_or_load(key)?;
        Ok(to_avro_schema(&schema))
    }

    /// Generate a JSON Schema document for the given key.
    pub fn to_json_schema(&self, key: &str) -> Result<Value, SchemaError> {
        let schema = self.get_or_load(key)?;
        Ok(to_json_schema(&schema))
    }

    // ── Existing methods ──────────────────────────────────────────

    /// Look up a schema by key (loading from disk if needed) and validate a DataFrame against it.
    pub fn validate_dataframe(
        &self,
        key: &str,
        df: &polars::prelude::DataFrame,
    ) -> Result<super::validation::ValidationReport, SchemaError> {
        let schema = self.get_or_load(key)?;
        validate_dataframe(&schema, df)
    }

    pub fn create_table_string(
        &self,
        key: &str,
        dialect: SqlDialect,
        table_schema: &str,
    ) -> Result<String, SchemaError> {
        let schema = self.get_or_load(key)?;
        let columns_sql: Vec<String> = schema
            .columns
            .iter()
            .map(|col| col.to_sql_column(dialect))
            .collect();
        Ok(format!(
            "CREATE TABLE IF NOT EXISTS {}.{} ({})",
            table_schema,
            schema.name,
            columns_sql.join(", ")
        ))
    }
}

// ── Compatibility checking ────────────────────────────────────────

/// Check whether `new` is compatible with `old` according to `new.compatibility`.
fn check_compatibility(old: &TableSchema, new: &TableSchema) -> Result<(), SchemaError> {
    match new.compatibility {
        CompatibilityMode::None => Ok(()),
        CompatibilityMode::Backward => check_backward(old, new),
        CompatibilityMode::Forward => check_forward(old, new),
        CompatibilityMode::Full => {
            check_backward(old, new)?;
            check_forward(old, new)
        }
    }
}

/// BACKWARD: new schema can read old data.
/// All columns in the old schema must exist in the new schema with compatible types.
/// New columns are allowed (they'll be null for old data).
fn check_backward(old: &TableSchema, new: &TableSchema) -> Result<(), SchemaError> {
    let new_cols: HashMap<&str, &polars::prelude::DataType> = new
        .columns
        .iter()
        .map(|c| (c.name.as_str(), &c.dtype))
        .collect();

    for old_col in &old.columns {
        match new_cols.get(old_col.name.as_str()) {
            None => {
                return Err(SchemaError::Incompatible(format!(
                    "BACKWARD: column '{}' was removed (old data still has it)",
                    old_col.name
                )));
            }
            Some(new_dtype) if **new_dtype != old_col.dtype => {
                return Err(SchemaError::Incompatible(format!(
                    "BACKWARD: column '{}' type changed from {:?} to {:?}",
                    old_col.name, old_col.dtype, new_dtype
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

/// FORWARD: old schema can read new data.
/// All columns in the new schema must exist in the old schema with compatible types.
/// Removed columns are allowed (old consumers just won't see them).
fn check_forward(old: &TableSchema, new: &TableSchema) -> Result<(), SchemaError> {
    let old_cols: HashMap<&str, &polars::prelude::DataType> = old
        .columns
        .iter()
        .map(|c| (c.name.as_str(), &c.dtype))
        .collect();

    for new_col in &new.columns {
        match old_cols.get(new_col.name.as_str()) {
            None => {
                return Err(SchemaError::Incompatible(format!(
                    "FORWARD: column '{}' was added (old consumers can't read it)",
                    new_col.name
                )));
            }
            Some(old_dtype) if **old_dtype != new_col.dtype => {
                return Err(SchemaError::Incompatible(format!(
                    "FORWARD: column '{}' type changed from {:?} to {:?}",
                    new_col.name, old_dtype, new_col.dtype
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use polars::prelude::*;

    use super::*;
    use crate::ColumnDef;

    fn make_schema(cols: Vec<(&str, DataType)>) -> TableSchema {
        make_versioned_schema("test", 1, CompatibilityMode::None, cols)
    }

    fn make_versioned_schema(
        name: &str,
        version: u32,
        compat: CompatibilityMode,
        cols: Vec<(&str, DataType)>,
    ) -> TableSchema {
        TableSchema {
            name: name.to_string(),
            version,
            description: None,
            source: None,
            pattern: None,
            compatibility: compat,
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

    // ── Original registry tests ───────────────────────────────────

    #[test]
    fn registry_insert_and_get() {
        let registry = SchemaRegistry::new(std::env::temp_dir());
        let schema = make_schema(vec![("id", DataType::Int64)]);

        registry.insert("test", schema.clone());

        let retrieved = registry.get("test").unwrap();
        assert_eq!(retrieved.name, "test");
        assert_eq!(retrieved.columns.len(), 1);
    }

    #[test]
    fn registry_missing_key_returns_none() {
        let registry = SchemaRegistry::new(std::env::temp_dir());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn registry_validate_unknown_key_errors() {
        let registry = SchemaRegistry::new(std::env::temp_dir());
        let df = df! { "id" => &[1i64] }.unwrap();

        let err = registry.validate_dataframe("missing", &df).unwrap_err();
        assert!(matches!(err, SchemaError::IoError(_)));
    }

    #[test]
    fn registry_validate_success() {
        let registry = SchemaRegistry::new(std::env::temp_dir());
        registry.insert(
            "users",
            make_schema(vec![("id", DataType::Int64), ("name", DataType::String)]),
        );

        let df = df! {
            "id" => &[1i64, 2],
            "name" => &["a", "b"],
        }
        .unwrap();

        assert!(registry.validate_dataframe("users", &df).is_ok());
    }

    #[test]
    fn registry_validate_failure() {
        let registry = SchemaRegistry::new(std::env::temp_dir());
        registry.insert("users", make_schema(vec![("id", DataType::Int64)]));

        let df = df! { "id" => &["not_an_int"] }.unwrap();

        let err = registry.validate_dataframe("users", &df).unwrap_err();
        assert!(matches!(err, SchemaError::ValidationFailed(_)));
    }

    #[test]
    fn registry_load_from_file() {
        let dir = std::env::temp_dir().join("schema_test_load");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_schema.json");

        std::fs::write(
            &path,
            r#"{"name":"test","columns":[{"name":"x","type":"Float64"}]}"#,
        )
        .unwrap();

        let registry = SchemaRegistry::new(&dir);
        registry.load_from_file("test", &path).unwrap();

        let schema = registry.get("test").unwrap();
        assert_eq!(schema.columns[0].dtype, DataType::Float64);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_get_or_load_from_disk() {
        let dir = std::env::temp_dir().join("schema_test_lazy");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("orders.json"),
            r#"{"name":"orders","columns":[{"name":"id","type":"Int64"}]}"#,
        )
        .unwrap();

        let registry = SchemaRegistry::new(&dir);

        assert!(registry.get("orders").is_none());

        let schema = registry.get_or_load("orders").unwrap();
        assert_eq!(schema.name, "orders");
        assert_eq!(schema.columns[0].dtype, DataType::Int64);

        assert!(registry.get("orders").is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_get_or_load_missing_file_errors() {
        let dir = std::env::temp_dir().join("schema_test_missing");
        std::fs::create_dir_all(&dir).unwrap();

        let registry = SchemaRegistry::new(&dir);
        let err = registry.get_or_load("nonexistent").unwrap_err();
        assert!(matches!(err, SchemaError::IoError(_)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Version tracking tests ────────────────────────────────────

    #[test]
    fn version_tracking() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let v1 = make_versioned_schema(
            "orders",
            1,
            CompatibilityMode::None,
            vec![("id", DataType::Int64)],
        );
        let v2 = make_versioned_schema(
            "orders",
            2,
            CompatibilityMode::None,
            vec![("id", DataType::Int64), ("name", DataType::String)],
        );

        registry.insert("orders", v1);
        registry.insert("orders", v2);

        assert_eq!(registry.list_versions("orders"), vec![1, 2]);

        let fetched_v1 = registry.get_version("orders", 1).unwrap();
        assert_eq!(fetched_v1.columns.len(), 1);

        let fetched_v2 = registry.get_version("orders", 2).unwrap();
        assert_eq!(fetched_v2.columns.len(), 2);

        // `get` returns the latest inserted
        let latest = registry.get("orders").unwrap();
        assert_eq!(latest.version, 2);
    }

    #[test]
    fn list_versions_empty() {
        let registry = SchemaRegistry::new(std::env::temp_dir());
        assert!(registry.list_versions("nothing").is_empty());
    }

    #[test]
    fn get_version_missing() {
        let registry = SchemaRegistry::new(std::env::temp_dir());
        assert!(registry.get_version("nothing", 1).is_none());
    }

    // ── Compatibility tests ───────────────────────────────────────

    #[test]
    fn backward_compatible_add_column() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let v1 = make_versioned_schema(
            "orders",
            1,
            CompatibilityMode::Backward,
            vec![("id", DataType::Int64)],
        );
        registry.insert("orders", v1);

        // v2 adds a column — backward compatible (old data just has null)
        let v2 = make_versioned_schema(
            "orders",
            2,
            CompatibilityMode::Backward,
            vec![("id", DataType::Int64), ("name", DataType::String)],
        );
        assert!(registry.register_version("orders", v2).is_ok());
    }

    #[test]
    fn backward_incompatible_remove_column() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let v1 = make_versioned_schema(
            "orders",
            1,
            CompatibilityMode::Backward,
            vec![("id", DataType::Int64), ("name", DataType::String)],
        );
        registry.insert("orders", v1);

        // v2 removes "name" — backward incompatible (old data has it)
        let v2 = make_versioned_schema(
            "orders",
            2,
            CompatibilityMode::Backward,
            vec![("id", DataType::Int64)],
        );
        let err = registry.register_version("orders", v2).unwrap_err();
        assert!(matches!(err, SchemaError::Incompatible(_)));
    }

    #[test]
    fn backward_incompatible_type_change() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let v1 = make_versioned_schema(
            "orders",
            1,
            CompatibilityMode::Backward,
            vec![("id", DataType::Int64)],
        );
        registry.insert("orders", v1);

        // v2 changes id from Int64 to String — backward incompatible
        let v2 = make_versioned_schema(
            "orders",
            2,
            CompatibilityMode::Backward,
            vec![("id", DataType::String)],
        );
        let err = registry.register_version("orders", v2).unwrap_err();
        assert!(matches!(err, SchemaError::Incompatible(_)));
    }

    #[test]
    fn forward_compatible_remove_column() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let v1 = make_versioned_schema(
            "orders",
            1,
            CompatibilityMode::Forward,
            vec![("id", DataType::Int64), ("name", DataType::String)],
        );
        registry.insert("orders", v1);

        // v2 removes "name" — forward compatible (old consumers just ignore)
        let v2 = make_versioned_schema(
            "orders",
            2,
            CompatibilityMode::Forward,
            vec![("id", DataType::Int64)],
        );
        assert!(registry.register_version("orders", v2).is_ok());
    }

    #[test]
    fn forward_incompatible_add_column() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let v1 = make_versioned_schema(
            "orders",
            1,
            CompatibilityMode::Forward,
            vec![("id", DataType::Int64)],
        );
        registry.insert("orders", v1);

        // v2 adds a column — forward incompatible (old consumers can't read it)
        let v2 = make_versioned_schema(
            "orders",
            2,
            CompatibilityMode::Forward,
            vec![("id", DataType::Int64), ("name", DataType::String)],
        );
        let err = registry.register_version("orders", v2).unwrap_err();
        assert!(matches!(err, SchemaError::Incompatible(_)));
    }

    #[test]
    fn full_compatible_no_change() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let v1 = make_versioned_schema(
            "orders",
            1,
            CompatibilityMode::Full,
            vec![("id", DataType::Int64)],
        );
        registry.insert("orders", v1);

        // v2 same columns — full compatible
        let v2 = make_versioned_schema(
            "orders",
            2,
            CompatibilityMode::Full,
            vec![("id", DataType::Int64)],
        );
        assert!(registry.register_version("orders", v2).is_ok());
    }

    #[test]
    fn none_compatibility_allows_anything() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let v1 = make_versioned_schema(
            "orders",
            1,
            CompatibilityMode::None,
            vec![("id", DataType::Int64)],
        );
        registry.insert("orders", v1);

        // v2 completely different — allowed with None
        let v2 = make_versioned_schema(
            "orders",
            2,
            CompatibilityMode::None,
            vec![("total", DataType::Float64)],
        );
        assert!(registry.register_version("orders", v2).is_ok());
    }

    #[test]
    fn first_version_always_registers() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let v1 = make_versioned_schema(
            "new_table",
            1,
            CompatibilityMode::Full,
            vec![("id", DataType::Int64)],
        );
        // No previous version — should succeed regardless of compat mode
        assert!(registry.register_version("new_table", v1).is_ok());
    }

    // ── Format converter tests ────────────────────────────────────

    #[test]
    fn registry_to_avro_schema() {
        let registry = SchemaRegistry::new(std::env::temp_dir());
        registry.insert(
            "users",
            make_schema(vec![("id", DataType::Int64), ("name", DataType::String)]),
        );

        let avro = registry.to_avro_schema("users").unwrap();
        assert_eq!(avro["type"], "record");
        assert_eq!(avro["name"], "test"); // name comes from TableSchema.name
        assert_eq!(avro["fields"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn registry_to_json_schema() {
        let registry = SchemaRegistry::new(std::env::temp_dir());
        registry.insert(
            "users",
            make_schema(vec![("id", DataType::Int64), ("name", DataType::String)]),
        );

        let js = registry.to_json_schema("users").unwrap();
        assert_eq!(js["type"], "object");
        assert_eq!(js["title"], "test");
        assert!(js["properties"]["id"].is_object());
    }

    // ── DV2.0 parent reference tests ─────────────────────────────

    #[test]
    fn satellite_requires_registered_parent() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let sat = TableSchema {
            name: "sat_customer_demo".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: vec![],
            dv_metadata: Some(crate::DvMetadata::Satellite {
                parent: "hub_customer".to_string(),
            }),
            extra_column_policy: crate::ExtraColumnPolicy::Allow,
        };

        // Parent not registered yet — should fail
        let err = registry
            .register_version("sat_customer_demo", sat.clone())
            .unwrap_err();
        assert!(matches!(err, SchemaError::InvalidDvSchema(_)));
        let msg = err.to_string();
        assert!(msg.contains("not registered"), "got: {msg}");
    }

    #[test]
    fn satellite_succeeds_with_hub_parent() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        // Register the hub first
        let hub = TableSchema {
            name: "hub_customer".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: vec![ColumnDef {
                name: "customer_id".to_string(),
                dtype: DataType::String,
                constraints: None,
            }],
            dv_metadata: Some(crate::DvMetadata::Hub {
                business_keys: vec!["customer_id".to_string()],
            }),
            extra_column_policy: crate::ExtraColumnPolicy::Reject,
        };
        registry.insert("hub_customer", hub);

        // Now register the satellite
        let sat = TableSchema {
            name: "sat_customer_demo".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: vec![],
            dv_metadata: Some(crate::DvMetadata::Satellite {
                parent: "hub_customer".to_string(),
            }),
            extra_column_policy: crate::ExtraColumnPolicy::Allow,
        };

        assert!(registry.register_version("sat_customer_demo", sat).is_ok());
    }

    #[test]
    fn satellite_rejects_legacy_parent() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        // Register a legacy schema (no dv_metadata)
        let legacy = make_schema(vec![("id", DataType::Int64)]);
        registry.insert("legacy_table", legacy);

        // Try to register a satellite pointing to the legacy schema
        let sat = TableSchema {
            name: "sat_bad".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: vec![],
            dv_metadata: Some(crate::DvMetadata::Satellite {
                parent: "legacy_table".to_string(),
            }),
            extra_column_policy: crate::ExtraColumnPolicy::Allow,
        };

        let err = registry.register_version("sat_bad", sat).unwrap_err();
        assert!(matches!(err, SchemaError::InvalidDvSchema(_)));
        let msg = err.to_string();
        assert!(msg.contains("must be a hub or link"), "got: {msg}");
    }

    #[test]
    fn hub_and_link_register_without_parent() {
        let registry = SchemaRegistry::new(std::env::temp_dir());

        let hub = TableSchema {
            name: "hub_product".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: vec![],
            dv_metadata: Some(crate::DvMetadata::Hub {
                business_keys: vec!["product_id".to_string()],
            }),
            extra_column_policy: crate::ExtraColumnPolicy::Reject,
        };
        assert!(registry.register_version("hub_product", hub).is_ok());

        let link = TableSchema {
            name: "lnk_test".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: vec![],
            dv_metadata: Some(crate::DvMetadata::Link {
                linked_hubs: vec!["hub_a".to_string(), "hub_b".to_string()],
            }),
            extra_column_policy: crate::ExtraColumnPolicy::Reject,
        };
        assert!(registry.register_version("lnk_test", link).is_ok());
    }
}
