use polars::prelude::*;
use sha2::{Digest, Sha256};

use super::types::{DvMetadata, SchemaError, TableSchema};

/// Separator between concatenated field values, so `"AB" + "C"` and
/// `"A" + "BC"` produce different inputs.
const FIELD_SEPARATOR: &str = "||";

/// Placeholder substituted for a NULL value when hashing with
/// [`NullPolicy::Sentinel`]. Distinct from an empty cell only in intent — for
/// change detection that is sufficient.
const NULL_SENTINEL: &str = "^^";

/// How a NULL in one of the hashed columns is treated.
#[derive(Clone, Copy)]
enum NullPolicy {
    /// Any NULL makes the whole hash NULL — correct for a *key* (a row with no
    /// business key has no hub).
    Propagate,
    /// NULLs are replaced with [`NULL_SENTINEL`] and still hashed — correct for a
    /// *hashdiff*, where a NULL attribute is a normal, change-detectable value.
    Sentinel,
}

/// SHA-256 of `input`, as a 64-char lowercase hex string.
fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Concatenate the given columns per row and SHA-256 them, honouring `policy`
/// for NULLs. The single engine behind both the key and hashdiff helpers.
fn hash_columns(
    df: &DataFrame,
    columns: &[&str],
    hash_col_name: &str,
    policy: NullPolicy,
) -> Result<Series, SchemaError> {
    let height = df.height();
    let mut hashes: Vec<Option<String>> = Vec::with_capacity(height);

    let series: Vec<Series> = columns
        .iter()
        .map(|&name| {
            df.column(name)
                .map(|c| c.as_materialized_series().clone())
                .map_err(|_| {
                    SchemaError::InvalidDvSchema(format!("column '{name}' not found in DataFrame"))
                })
        })
        .collect::<Result<_, _>>()?;

    let strs: Vec<StringChunked> = series
        .iter()
        .map(|s| s.cast(&DataType::String).unwrap_or_else(|_| s.clone()))
        .map(|s| s.str().unwrap().clone())
        .collect();

    for row_idx in 0..height {
        let mut any_null = false;
        let mut parts: Vec<String> = Vec::with_capacity(columns.len());

        for ca in &strs {
            match ca.get(row_idx) {
                Some(val) => parts.push(val.to_string()),
                None => match policy {
                    NullPolicy::Propagate => {
                        any_null = true;
                        break;
                    }
                    NullPolicy::Sentinel => parts.push(NULL_SENTINEL.to_string()),
                },
            }
        }

        if any_null {
            hashes.push(None);
        } else {
            hashes.push(Some(sha256_hex(&parts.join(FIELD_SEPARATOR))));
        }
    }

    Ok(StringChunked::new(hash_col_name.into(), &hashes).into_series())
}

/// Compute a hash key column from one or more key columns in a DataFrame.
///
/// For each row, concatenates the string representations of the key columns
/// with `||` as separator, then takes the SHA-256 hash (lowercase hex).
/// If any key column value is null for a row, the hash is null for that row
/// (a row with no business key has no hub).
pub fn compute_hash_key(
    df: &DataFrame,
    key_columns: &[&str],
    hash_col_name: &str,
) -> Result<Series, SchemaError> {
    hash_columns(df, key_columns, hash_col_name, NullPolicy::Propagate)
}

/// Compute a `hashdiff` (row hash) over the given descriptive columns.
///
/// Unlike [`compute_hash_key`], NULLs are substituted with a sentinel and still
/// hashed — a NULL attribute is a normal value to track for change detection, so
/// the result is never NULL. This is the per-row content fingerprint used to tell
/// whether an otherwise-identical business key carries changed data.
pub fn compute_hashdiff(
    df: &DataFrame,
    columns: &[&str],
    hash_col_name: &str,
) -> Result<Series, SchemaError> {
    hash_columns(df, columns, hash_col_name, NullPolicy::Sentinel)
}

/// Compute the hub hash key for a DataFrame using the schema's business keys.
///
/// Extracts business key column names from `DvMetadata::Hub` and delegates
/// to `compute_hash_key`.
pub fn compute_hub_hash_key(df: &DataFrame, schema: &TableSchema) -> Result<Series, SchemaError> {
    let (business_keys, hash_col_name) = match &schema.dv_metadata {
        Some(DvMetadata::Hub { business_keys }) => {
            let hash_col = format!("{}_hk", schema.name);
            (business_keys.clone(), hash_col)
        }
        _ => {
            return Err(SchemaError::InvalidDvSchema(
                "compute_hub_hash_key requires a hub schema".to_string(),
            ));
        }
    };

    let key_refs: Vec<&str> = business_keys.iter().map(|s| s.as_str()).collect();
    compute_hash_key(df, &key_refs, &hash_col_name)
}

/// Compute the link hash key for a DataFrame using the linked hub hash key columns.
///
/// Extracts hub hash key column names from `DvMetadata::Link` and delegates
/// to `compute_hash_key`.
pub fn compute_link_hash_key(df: &DataFrame, schema: &TableSchema) -> Result<Series, SchemaError> {
    let (hub_hk_cols, hash_col_name) = match &schema.dv_metadata {
        Some(DvMetadata::Link { linked_hubs }) => {
            let hk_cols: Vec<String> = linked_hubs.iter().map(|h| format!("{h}_hk")).collect();
            let hash_col = format!("{}_hk", schema.name);
            (hk_cols, hash_col)
        }
        _ => {
            return Err(SchemaError::InvalidDvSchema(
                "compute_link_hash_key requires a link schema".to_string(),
            ));
        }
    };

    let key_refs: Vec<&str> = hub_hk_cols.iter().map(|s| s.as_str()).collect();
    compute_hash_key(df, &key_refs, &hash_col_name)
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColumnDef, CompatibilityMode, ExtraColumnPolicy};

    #[test]
    fn hash_key_deterministic() {
        let df = df! {
            "customer_id" => &["C001", "C002", "C003"],
        }
        .unwrap();

        let result = compute_hash_key(&df, &["customer_id"], "hub_customer_hk").unwrap();
        let ca = result.str().unwrap();

        // Verify deterministic: same input → same output
        let result2 = compute_hash_key(&df, &["customer_id"], "hub_customer_hk").unwrap();
        let ca2 = result2.str().unwrap();

        for i in 0..3 {
            assert_eq!(ca.get(i), ca2.get(i));
            assert!(ca.get(i).is_some()); // no nulls
        }

        // Verify it's a valid 64-char hex string (SHA-256)
        let hash = ca.get(0).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hashdiff_hashes_nulls_instead_of_propagating() {
        let df = df! {
            "a" => &[Some("x"), None],
            "b" => &[Some("1"), Some("2")],
        }
        .unwrap();

        let result = compute_hashdiff(&df, &["a", "b"], "hashdiff").unwrap();
        let ca = result.str().unwrap();

        // Neither row is null — a NULL attribute is a tracked value, not absence.
        assert!(ca.get(0).is_some());
        assert!(ca.get(1).is_some());
        // The two rows differ, so their hashdiffs differ.
        assert_ne!(ca.get(0), ca.get(1));
    }

    #[test]
    fn hashdiff_is_stable_for_equal_rows() {
        let df = df! { "a" => &["x", "x"], "b" => &["1", "1"] }.unwrap();
        let ca = compute_hashdiff(&df, &["a", "b"], "hashdiff").unwrap();
        let s = ca.str().unwrap();
        assert_eq!(s.get(0), s.get(1));
    }

    #[test]
    fn hash_key_null_propagation() {
        let df = df! {
            "a" => &[Some("x"), None, Some("z")],
            "b" => &[Some("1"), Some("2"), Some("3")],
        }
        .unwrap();

        let result = compute_hash_key(&df, &["a", "b"], "hk").unwrap();
        let ca = result.str().unwrap();

        assert!(ca.get(0).is_some()); // "x||1" → hash
        assert!(ca.get(1).is_none()); // null in key → null hash
        assert!(ca.get(2).is_some()); // "z||3" → hash
    }

    #[test]
    fn hash_key_multi_column_separator() {
        let df = df! {
            "a" => &["AB", "A"],
            "b" => &["C", "BC"],
        }
        .unwrap();

        let result = compute_hash_key(&df, &["a", "b"], "hk").unwrap();
        let ca = result.str().unwrap();

        // "AB||C" != "A||BC" → different hashes
        assert_ne!(ca.get(0), ca.get(1));
    }

    #[test]
    fn hash_key_missing_column_errors() {
        let df = df! { "a" => &["x"] }.unwrap();
        let err = compute_hash_key(&df, &["nonexistent"], "hk").unwrap_err();
        assert!(matches!(err, SchemaError::InvalidDvSchema(_)));
    }

    #[test]
    fn compute_hub_hash_key_works() {
        let schema = TableSchema {
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
            dv_metadata: Some(DvMetadata::Hub {
                business_keys: vec!["customer_id".to_string()],
            }),
            extra_column_policy: ExtraColumnPolicy::Reject,
        };

        let df = df! { "customer_id" => &["C001", "C002"] }.unwrap();
        let result = compute_hub_hash_key(&df, &schema).unwrap();

        assert_eq!(result.name().as_str(), "hub_customer_hk");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn compute_link_hash_key_works() {
        let schema = TableSchema {
            name: "lnk_customer_order".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: vec![],
            dv_metadata: Some(DvMetadata::Link {
                linked_hubs: vec!["hub_customer".to_string(), "hub_order".to_string()],
            }),
            extra_column_policy: ExtraColumnPolicy::Reject,
        };

        let df = df! {
            "hub_customer_hk" => &["abc123", "def456"],
            "hub_order_hk" => &["ord001", "ord002"],
        }
        .unwrap();

        let result = compute_link_hash_key(&df, &schema).unwrap();

        assert_eq!(result.name().as_str(), "lnk_customer_order_hk");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn compute_hub_hash_key_rejects_non_hub() {
        let schema = TableSchema {
            name: "test".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: None,
            columns: vec![],
            dv_metadata: None,
            extra_column_policy: ExtraColumnPolicy::default(),
        };

        let df = df! { "x" => &[1i64] }.unwrap();
        let err = compute_hub_hash_key(&df, &schema).unwrap_err();
        assert!(matches!(err, SchemaError::InvalidDvSchema(_)));
    }
}
