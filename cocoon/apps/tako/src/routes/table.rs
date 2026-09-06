//! `POST /table` — create the warehouse staging table for a schema.
//!
//! Table creation (DDL) is deliberately separate from data ingestion (`/upload`,
//! which only `COPY`s). That keeps each request a single warehouse statement —
//! no multi-statement transaction to coordinate — and means an upload to a
//! missing table fails fast instead of silently creating it.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;
use utoipa::{IntoParams, ToSchema};

use database::{DatabaseError, Identifier};
use schema::{SchemaError, SchemaRegistry, SqlDialect, enriched_columns};

use crate::error::{ApiError, ErrorResponse};
use crate::state::AppState;

/// Schema all tables live in. Single source of truth for the qualified name.
const STAGE_SCHEMA: &str = "stage";

#[derive(Deserialize, IntoParams)]
pub struct CreateTableParams {
    /// Target table name (required). Validated as a strict SQL identifier.
    pub table: Option<String>,
    /// Schema-registry key defining the columns (defaults to `stage`).
    pub schema: Option<String>,
}

fn db_type_to_dialect(db_type: &database::DBType) -> SqlDialect {
    match db_type {
        database::DBType::DuckDb => SqlDialect::DuckDb,
        database::DBType::Postgres => SqlDialect::Postgres,
        database::DBType::Snowflake => SqlDialect::Snowflake,
        // Any other feature-gated backend (e.g. SQLite) has no dialect of its
        // own; fall back to Snowflake's SQL generation.
        #[allow(unreachable_patterns)]
        _ => SqlDialect::Snowflake,
    }
}

fn build_create_table_sql(
    registry: &SchemaRegistry,
    schema_key: &str,
    dialect: SqlDialect,
    db_schema: &str,
    table: &Identifier,
) -> Result<String, ApiError> {
    let schema = registry.get_or_load(schema_key).map_err(|e| match e {
        SchemaError::NotFound(k) => ApiError::BadRequest(format!("Schema not found: {k}")),
        other => ApiError::internal("Failed to load schema from registry", other),
    })?;
    // For a DV-enriched schema this adds the hash key, hashdiff and load metadata
    // columns (in the canonical order the upload also writes); for a plain schema
    // it is just the declared columns.
    let columns_sql: Vec<String> = enriched_columns(&schema)
        .iter()
        .map(|col| col.to_sql_column(dialect))
        .collect();
    Ok(format!(
        "CREATE TABLE IF NOT EXISTS {db_schema}.{table} ({})",
        columns_sql.join(", ")
    ))
}

/// Create the `stage.<table>` table from the named schema. Idempotent
/// (`CREATE TABLE IF NOT EXISTS`), so it is safe to call repeatedly.
#[utoipa::path(
    post,
    path = "/table",
    tag = "Table",
    params(CreateTableParams),
    responses(
        (status = 201, description = "Table created (or already existed)", body = serde_json::Value),
        (status = 400, description = "Missing or invalid table name", body = ErrorResponse),
        (status = 500, description = "Warehouse error", body = ErrorResponse)
    )
)]
pub async fn create_table(
    State(state): State<AppState>,
    Query(params): Query<CreateTableParams>,
) -> Result<Response, ApiError> {
    let table_name = params
        .table
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("Missing required query parameter: table".into()))?;
    let table = Identifier::new(table_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid table name {table_name:?}: {e}")))?;
    let schema_key = params.schema.as_deref().unwrap_or("stage");

    let db_type = *state.db.db_type();
    let dialect = db_type_to_dialect(&db_type);
    let create_sql = build_create_table_sql(
        &state.schema_registry,
        schema_key,
        dialect,
        STAGE_SCHEMA,
        &table,
    )?;

    info!(sql = %create_sql, table = %table, schema = %schema_key, "Creating table");

    // Blocking DB work off the async runtime (the sync warehouse drivers manage
    // their own internal runtimes).
    let pool = state.db.clone();
    tokio::task::spawn_blocking(move || -> Result<(), ApiError> {
        let conn = pool
            .get()
            .map_err(|e| ApiError::internal("Failed to get DB connection from pool", e))?;
        // DuckDB needs the schema created first; for the warehouse backends the
        // `stage` schema is provisioned out of band.
        if matches!(db_type, database::DBType::DuckDb) {
            conn.execute("CREATE SCHEMA IF NOT EXISTS stage", &[])
                .map_err(|e| ApiError::internal("Failed to create schema", e))?;
        }
        conn.execute(&create_sql, &[])
            .map_err(|e| ApiError::internal(&format!("CREATE TABLE failed: {create_sql}"), e))?;
        Ok(())
    })
    .await
    .map_err(|e| ApiError::internal("Create-table task panicked", e))??;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "message": "Table ready",
            "table": table.as_str(),
            "schema": schema_key,
        })),
    )
        .into_response())
}

// ── GET: list / inspect tables ────────────────────────────────────────────

/// Top-level view returned by `GET /table` — the tables in the staging schema.
#[derive(Serialize, ToSchema)]
pub(crate) struct TableList {
    schema: String,
    tables: Vec<String>,
}

/// One column in a table's breakdown.
#[derive(Serialize, ToSchema)]
pub(crate) struct ColumnView {
    name: String,
    data_type: String,
    nullable: bool,
    position: usize,
}

/// Rich view returned by `GET /table/{name}`.
#[derive(Serialize, ToSchema)]
pub(crate) struct TableDetail {
    schema: String,
    table: String,
    columns: Vec<ColumnView>,
}

/// List every table in the staging schema (names only — a top-level overview).
#[utoipa::path(
    get,
    path = "/table",
    tag = "Table",
    responses((status = 200, description = "Tables in the staging schema", body = TableList))
)]
pub async fn list_tables(State(state): State<AppState>) -> Result<Response, ApiError> {
    let pool = state.db.clone();
    // `UPPER(table_schema)` normalises the casing difference between warehouses
    // (Snowflake upper-cases unquoted identifiers, Postgres lower-cases them).
    let rows = tokio::task::spawn_blocking(move || {
        pool.query(
            "SELECT table_name FROM information_schema.tables \
             WHERE UPPER(table_schema) = 'STAGE' ORDER BY table_name",
            &[],
        )
    })
    .await
    .map_err(|e| ApiError::internal("List-tables task panicked", e))?
    .map_err(|e| ApiError::internal("Failed to list tables", e))?;

    // Each row has a single column; take its value without depending on the
    // column-name casing the warehouse returns.
    let tables: Vec<String> = rows
        .into_iter()
        .filter_map(|row| {
            row.into_values()
                .next()
                .and_then(|v| v.as_str().map(str::to_string))
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(TableList {
            schema: STAGE_SCHEMA.to_string(),
            tables,
        }),
    )
        .into_response())
}

/// Return a single table's column breakdown, or 404 if it does not exist.
#[utoipa::path(
    get,
    path = "/table/{name}",
    tag = "Table",
    params(("name" = String, Path, description = "Table name within the staging schema")),
    responses(
        (status = 200, description = "Column breakdown of the table", body = TableDetail),
        (status = 400, description = "Invalid table name", body = ErrorResponse),
        (status = 404, description = "Table not found", body = ErrorResponse)
    )
)]
pub async fn get_table(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    let table = Identifier::new(&name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid table name {name:?}: {e}")))?;
    let qualified = format!("{STAGE_SCHEMA}.{table}");

    let pool = state.db.clone();
    let q = qualified.clone();
    let schema = tokio::task::spawn_blocking(move || pool.get_table_schema(&q))
        .await
        .map_err(|e| ApiError::internal("Get-table task panicked", e))?
        .map_err(|e| match e {
            DatabaseError::NotFound => ApiError::NotFound(format!("Table not found: {qualified}")),
            other => ApiError::internal("Failed to read table schema", other),
        })?;

    let mut columns: Vec<ColumnView> = schema
        .columns
        .iter()
        .map(|c| ColumnView {
            name: c.name.clone(),
            data_type: c.data_type.clone(),
            nullable: c.is_nullable,
            position: c.ordinal_position,
        })
        .collect();
    columns.sort_by_key(|c| c.position);

    Ok((
        StatusCode::OK,
        Json(TableDetail {
            schema: STAGE_SCHEMA.to_string(),
            table: table.as_str().to_string(),
            columns,
        }),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialect_mapping() {
        assert!(matches!(
            db_type_to_dialect(&database::DBType::DuckDb),
            SqlDialect::DuckDb
        ));
        assert!(matches!(
            db_type_to_dialect(&database::DBType::Postgres),
            SqlDialect::Postgres
        ));
    }
}
