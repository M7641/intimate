pub mod avro;
pub mod dialect;
pub mod dv;
pub mod hash;
pub mod json_schema;
mod registry;
mod types;
mod validation;

pub use avro::{dtype_to_avro, to_avro_schema};
pub use dialect::{SqlDialect, dtype_to_sql};
pub use dv::{enrich_dataframe, enriched_columns};
pub use hash::{compute_hash_key, compute_hashdiff, compute_hub_hash_key, compute_link_hash_key};
pub use json_schema::{dtype_to_json_schema, to_json_schema};
pub use registry::SchemaRegistry;
pub use types::{
    ColumnConstraints, ColumnDef, CompatibilityMode, DvMetadata, DvTableType, ExtraColumnPolicy,
    IngestionPattern, KeyDeclarations, Relationship, SchemaError, TableSchema, parse_polars_dtype,
};
pub use validation::{OrderMismatch, TypeMismatch, ValidationReport, validate_dataframe};
