//! Data Vault raw-vault enrichment for an uploaded table.
//!
//! A schema opts in by declaring a business key (see
//! [`TableSchema::business_key`]). Its rows are then enriched, **insert-only**,
//! with the standard DV metadata so every load is traceable and deduplicable
//! downstream rather than a silent duplicate:
//!
//! - `<entity>_hk` — SHA-256 of the business key column(s) (the hub key);
//! - `hashdiff` — SHA-256 of the descriptive columns (per-row change detection);
//! - `load_date` — when the batch was loaded;
//! - `record_source` — provenance;
//! - `load_id` — a unique id shared by every row of one upload.
//!
//! [`enriched_columns`] is the single source of truth for the column set and its
//! order; both the table DDL and the written Parquet derive from it, so a
//! position-matched `COPY` (DuckDB, Redshift) and a name-matched one (Snowflake)
//! both line up.

use polars::prelude::*;

use crate::hash::{compute_hash_key, compute_hashdiff};
use crate::types::{
    ColumnDef, SchemaError, TableSchema, hash_key_column, hashdiff_column, load_date_column,
    load_id_column, record_source_column,
};

fn polars_err(e: PolarsError) -> SchemaError {
    SchemaError::DvEnrichment(e.to_string())
}

/// The full column set for a table, in canonical order.
///
/// For a DV-enriched schema (a declared business key) this is
/// `[<entity>_hk, …business columns…, hashdiff, load_date, record_source,
/// load_id]`. For a plain schema it is just the declared columns, unchanged.
pub fn enriched_columns(schema: &TableSchema) -> Vec<ColumnDef> {
    if !schema.is_dv_enriched() {
        return schema.columns.clone();
    }

    let mut cols = Vec::with_capacity(schema.columns.len() + 5);
    cols.push(hash_key_column(&schema.name));
    cols.extend(schema.columns.iter().cloned());
    cols.push(hashdiff_column());
    cols.push(load_date_column());
    cols.push(record_source_column());
    cols.push(load_id_column());
    cols
}

/// Enrich a validated business `DataFrame` with the DV metadata columns.
///
/// A no-op (returns the frame unchanged) for a schema with no business key.
/// Otherwise it computes the hash key and hashdiff, stamps the per-batch
/// `load_date` / `record_source` / `load_id`, and returns the frame reordered to
/// match [`enriched_columns`] exactly.
pub fn enrich_dataframe(
    df: DataFrame,
    schema: &TableSchema,
    record_source: &str,
    load_id: &str,
    load_timestamp_micros: i64,
) -> Result<DataFrame, SchemaError> {
    let Some(business_key) = schema.business_key() else {
        return Ok(df);
    };

    let mut df = df;
    let height = df.height();

    // Hub hash key: SHA-256 of the business key column(s), null-propagating.
    let bk_refs: Vec<&str> = business_key.iter().map(|s| s.as_str()).collect();
    let hk = compute_hash_key(&df, &bk_refs, &format!("{}_hk", schema.name))?;

    // Row hash: SHA-256 over every declared business column (nulls sentinelled).
    let desc_refs: Vec<&str> = schema.columns.iter().map(|c| c.name.as_str()).collect();
    let hashdiff = compute_hashdiff(&df, &desc_refs, "hashdiff")?;

    // Per-batch constant metadata.
    let load_date = Int64Chunked::full("load_date".into(), load_timestamp_micros, height)
        .into_series()
        .cast(&DataType::Datetime(TimeUnit::Microseconds, None))
        .map_err(polars_err)?;
    let record_source_s =
        StringChunked::full("record_source".into(), record_source, height).into_series();
    let load_id_s = StringChunked::full("load_id".into(), load_id, height).into_series();

    df.with_column(hk).map_err(polars_err)?;
    df.with_column(hashdiff).map_err(polars_err)?;
    df.with_column(load_date).map_err(polars_err)?;
    df.with_column(record_source_s).map_err(polars_err)?;
    df.with_column(load_id_s).map_err(polars_err)?;

    // Reorder to the canonical column order so the Parquet matches the table DDL
    // (DuckDB/Redshift match a Parquet COPY by position).
    let order: Vec<String> = enriched_columns(schema)
        .into_iter()
        .map(|c| c.name)
        .collect();
    df.select(order).map_err(polars_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::KeyDeclarations;
    use crate::{CompatibilityMode, ExtraColumnPolicy};
    use std::collections::HashMap;

    fn col(name: &str, dtype: DataType) -> ColumnDef {
        ColumnDef {
            name: name.to_string(),
            dtype,
            constraints: None,
        }
    }

    fn dv_schema() -> TableSchema {
        TableSchema {
            name: "customer".to_string(),
            version: 1,
            description: None,
            source: None,
            pattern: None,
            compatibility: CompatibilityMode::None,
            keys: Some(KeyDeclarations {
                business_key: vec!["customer_id".to_string()],
                relationships: HashMap::new(),
            }),
            columns: vec![
                col("customer_id", DataType::String),
                col("name", DataType::String),
            ],
            dv_metadata: None,
            extra_column_policy: ExtraColumnPolicy::Reject,
        }
    }

    fn plain_schema() -> TableSchema {
        let mut s = dv_schema();
        s.keys = None;
        s
    }

    #[test]
    fn enriched_columns_adds_dv_metadata_in_order() {
        let names: Vec<String> = enriched_columns(&dv_schema())
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(
            names,
            vec![
                "customer_hk",
                "customer_id",
                "name",
                "hashdiff",
                "load_date",
                "record_source",
                "load_id",
            ]
        );
    }

    #[test]
    fn enriched_columns_is_a_noop_without_business_key() {
        let names: Vec<String> = enriched_columns(&plain_schema())
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, vec!["customer_id", "name"]);
    }

    #[test]
    fn enrich_dataframe_matches_enriched_columns_order() {
        let df = df! {
            "customer_id" => &["C001", "C002"],
            "name" => &["Ada", "Grace"],
        }
        .unwrap();

        let out = enrich_dataframe(
            df,
            &dv_schema(),
            "tako/sample/customer",
            "load-1",
            1_700_000_000_000_000,
        )
        .unwrap();

        let got: Vec<&str> = out.get_column_names().iter().map(|s| s.as_str()).collect();
        let want: Vec<String> = enriched_columns(&dv_schema())
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(got, want);
        assert_eq!(out.height(), 2);

        // The metadata columns are populated and constant across the batch.
        let rs = out.column("record_source").unwrap().str().unwrap();
        assert_eq!(rs.get(0), Some("tako/sample/customer"));
        assert_eq!(rs.get(1), Some("tako/sample/customer"));
        let lid = out.column("load_id").unwrap().str().unwrap();
        assert_eq!(lid.get(0), Some("load-1"));
    }

    #[test]
    fn enrich_dataframe_is_a_noop_without_business_key() {
        let df = df! { "customer_id" => &["C001"], "name" => &["Ada"] }.unwrap();
        let out = enrich_dataframe(df, &plain_schema(), "x", "y", 0).unwrap();
        let got: Vec<&str> = out.get_column_names().iter().map(|s| s.as_str()).collect();
        assert_eq!(got, vec!["customer_id", "name"]);
    }
}
