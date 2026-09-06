//! Warehouse-side metadata registry for Data Vault satellites.
//!
//! tako lets a table created by `/upload` be "edited" by attaching satellites
//! that share the hub's business key (the README FAQ's Option A). This module
//! owns the registry that records which business keys (hubs) exist and which
//! satellites hang off each — the single source of truth for enforcing the
//! satellites-per-hub cap and, later, for generating the joined "edited" view.
//!
//! The registry always lives in the [`META_SCHEMA`] schema, regardless of the
//! schema an individual hub or satellite table is created in.

use database::ApiDbActions;

/// The schema the registry tables always live in.
///
/// Deliberately fixed (not configurable): the registry is an operational
/// concern of tako itself, so it lives in one well-known place rather than
/// being scattered per deployment.
const META_SCHEMA: &str = "sandpit";

fn create_schema_sql() -> String {
    format!("CREATE SCHEMA IF NOT EXISTS {META_SCHEMA}")
}

/// Identity of a hub: what a business key represents. One row per (schema, hub).
fn hub_registry_ddl() -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {META_SCHEMA}.hub_registry (\
         schema_name VARCHAR NOT NULL, \
         hub_name VARCHAR NOT NULL, \
         business_key_columns VARCHAR NOT NULL, \
         created_at TIMESTAMP NOT NULL, \
         PRIMARY KEY (schema_name, hub_name))"
    )
}

/// The satellites attached to a hub. `precedence` orders column-conflict
/// resolution when the edited view is built; `active` soft-deletes a satellite
/// without losing its history (the registry is operational metadata, not vault
/// data, so it is mutable — unlike the append-only tables it describes).
fn satellite_registry_ddl() -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {META_SCHEMA}.satellite_registry (\
         schema_name VARCHAR NOT NULL, \
         hub_name VARCHAR NOT NULL, \
         satellite_name VARCHAR NOT NULL, \
         precedence INTEGER NOT NULL, \
         record_source VARCHAR NOT NULL, \
         created_at TIMESTAMP NOT NULL, \
         active BOOLEAN NOT NULL DEFAULT TRUE, \
         PRIMARY KEY (schema_name, hub_name, satellite_name))"
    )
}

/// Initialise the warehouse for the satellite registry: create the `sandpit`
/// schema (idempotent) and the two registry tables. Safe to run repeatedly —
/// every statement is `IF NOT EXISTS`.
pub async fn init_db() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let backend = std::env::var("DATA_WAREHOUSE_TYPE").unwrap_or_else(|_| "duckdb".to_string());

    if backend == "duckdb" && std::env::var_os("DUCKDB_PATH").is_none() {
        tracing::warn!(
            "DATA_WAREHOUSE_TYPE=duckdb with no DUCKDB_PATH: the registry is created in an \
             in-memory database, so it neither persists nor is visible to the server. Set \
             DUCKDB_PATH to a file for a durable local registry."
        );
    }

    // One connection is enough for a one-shot init; a pool of 1 keeps it simple.
    let db = ApiDbActions::connect(&backend, Some(1))?;

    let statements = [
        create_schema_sql(),
        hub_registry_ddl(),
        satellite_registry_ddl(),
    ];

    // Run every statement on the SAME connection: an in-memory DuckDB gives each
    // pooled connection its own independent database, so creating the schema on
    // one and the tables on another would not see each other (README's local
    // DuckDB note). The blocking driver runs off the async runtime.
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = db.get()?;
        for sql in &statements {
            tracing::info!(sql = %sql, "registry init");
            conn.execute(sql, &[])?;
        }
        Ok(())
    })
    .await??;

    tracing::info!(
        schema = META_SCHEMA,
        "registry initialised (hub_registry, satellite_registry)"
    );
    Ok(())
}
