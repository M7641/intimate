use axum::extract::{Multipart, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::{Json, Response};
use polars::prelude::*;
use serde::Deserialize;
use serde_json::json;
use std::io::Cursor;
use tracing::info;
use utoipa::IntoParams;

use crate::error::{ApiError, ErrorResponse};
use crate::parse;
use crate::state::{AppState, StorageConfig};
use database::{Identifier, build_copy_sql};
use schema::{ExtraColumnPolicy, SchemaError, TableSchema, enrich_dataframe, validate_dataframe};

/// Build the object key used for uploading to blob storage.
///
/// For Local, the key is passed through unchanged (the local storage root
/// is already configured on the client).
/// For S3, the key is prefixed with `{bucket}/` so the object lands at
/// `s3://{data_lake}/{bucket}/{key}` inside the data-lake bucket.
fn build_upload_key(config: &StorageConfig, key: &str) -> String {
    match config {
        StorageConfig::Local { .. } => key.to_string(),
        StorageConfig::S3 { bucket, .. } => format!("{bucket}/{key}"),
    }
}

/// Build the full path for the warehouse COPY command.
fn build_copy_path(config: &StorageConfig, key: &str) -> String {
    match config {
        StorageConfig::Local { root } => format!("{root}/{key}"),
        StorageConfig::S3 { data_lake, bucket } => format!("s3://{data_lake}/{bucket}/{key}"),
    }
}

#[derive(Deserialize, IntoParams)]
pub struct UploadParams {
    /// Destination table. **Required** — the request fails with 400 if omitted
    /// (there is no fallback to the file name).
    pub table: Option<String>,
    /// Schema-registry key used to validate the file (defaults to `stage`).
    pub schema: Option<String>,
}

struct UploadedFile {
    file_name: String,
    data: bytes::Bytes,
}

async fn extract_file(mut multipart: Multipart) -> Result<UploadedFile, ApiError> {
    let field = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Invalid multipart data: {e}")))?
        .ok_or_else(|| ApiError::BadRequest("No file provided in request".into()))?;

    let file_name = field
        .file_name()
        .ok_or_else(|| ApiError::BadRequest("Missing file name in request".into()))?
        .to_string();

    let data = field
        .bytes()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to read file data: {e}")))?;

    if let Ok(Some(_)) = multipart.next_field().await {
        return Err(ApiError::BadRequest(
            "Only one file per request is allowed. Please upload files individually.".into(),
        ));
    }

    if data.len() > crate::limits::MAX_FILE_SIZE {
        return Err(ApiError::PayloadTooLarge(format!(
            "File {file_name} exceeds maximum size of {} bytes",
            crate::limits::MAX_FILE_SIZE
        )));
    }

    Ok(UploadedFile { file_name, data })
}

/// Reorder the parsed frame to the schema's column order and validate it.
///
/// Operates on an already-resolved [`TableSchema`] so the caller can reuse that
/// same schema for Data Vault enrichment without loading it twice.
fn validate_and_reorder(schema: &TableSchema, df: DataFrame) -> Result<DataFrame, ApiError> {
    // Build target column list: schema columns (in schema order) that exist in the DF
    let df_col_names: Vec<String> = df
        .get_column_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    let df_col_set: std::collections::HashSet<&str> =
        df_col_names.iter().map(|s| s.as_str()).collect();

    let mut target_cols: Vec<String> = schema
        .columns
        .iter()
        .filter(|c| df_col_set.contains(c.name.as_str()))
        .map(|c| c.name.clone())
        .collect();

    // If extras are allowed, append non-schema columns (preserving DF order)
    if schema.extra_column_policy == ExtraColumnPolicy::Allow {
        let schema_col_set: std::collections::HashSet<&str> =
            schema.columns.iter().map(|c| c.name.as_str()).collect();
        for name in &df_col_names {
            if !schema_col_set.contains(name.as_str()) {
                target_cols.push(name.clone());
            }
        }
    }

    let df = df
        .select(target_cols.iter().map(|s| s.as_str()))
        .map_err(|e| ApiError::internal("Failed to reorder DataFrame columns", e))?;

    // Validate types and missing columns (extras & order already resolved)
    validate_dataframe(schema, &df).map_err(|e| match e {
        SchemaError::ValidationFailed(report) => {
            ApiError::BadRequest(format!("Schema validation failed:\n{report}"))
        }
        other => ApiError::BadRequest(other.to_string()),
    })?;

    info!(schema = %schema.name, "Schema validation passed");

    Ok(df)
}

/// Serialize a `DataFrame` to Parquet bytes.
///
/// Synchronous and CPU-bound: callers must run it off the async runtime (it is
/// invoked from inside the `spawn_blocking` block in `upload_file`, alongside
/// the equally CPU-heavy parse and schema steps, so the whole decode pipeline
/// occupies a single blocking thread rather than the async workers).
fn to_parquet(mut df: DataFrame) -> Result<Vec<u8>, ApiError> {
    let mut buffer = Cursor::new(Vec::new());
    ParquetWriter::new(&mut buffer)
        .finish(&mut df)
        .map_err(|e| ApiError::internal("Failed to convert to Parquet", e))?;
    Ok(buffer.into_inner())
}

/// `COPY` the uploaded Parquet into the (already-existing) `stage.<table>`.
///
/// This is a single warehouse statement — no transaction, no DDL. The table must
/// already exist (create it via `POST /table`); a missing table simply makes the
/// `COPY` fail here, which is the intended behaviour.
async fn load_into_warehouse(
    state: &AppState,
    table: &Identifier,
    s3_path: &str,
) -> Result<String, ApiError> {
    let db_type = *state.db.db_type();
    let qualified_table = format!("stage.{table}");

    let copy_path = build_copy_path(&state.storage_config, s3_path);
    let copy_sql = build_copy_sql(&db_type, &qualified_table, &copy_path)
        .map_err(|e| ApiError::internal("Failed to build COPY statement", e))?;

    info!(sql = %copy_sql, "Generated COPY command for data loading");

    // Blocking DB work off the async runtime (the sync warehouse drivers manage
    // their own internal runtimes).
    let pool = state.db.clone();
    let copy = copy_sql.clone();
    tokio::task::spawn_blocking(move || -> Result<(), ApiError> {
        let conn = pool
            .get()
            .map_err(|e| ApiError::internal("Failed to get DB connection from pool", e))?;
        conn.execute(&copy, &[])
            .map_err(|e| ApiError::internal(&format!("COPY failed: {copy}"), e))?;
        Ok(())
    })
    .await
    .map_err(|e| ApiError::internal("Warehouse load task panicked", e))??;

    Ok(copy_sql)
}

/// Upload a data file (multipart `file` field) and `COPY` it into an existing
/// `stage.<table>`. The table must already exist (see `POST /table`).
#[utoipa::path(
    post,
    path = "/upload",
    tag = "Upload",
    params(UploadParams),
    request_body(
        content = String,
        description = "Multipart form with a single `file` field (CSV / Parquet / JSON / Avro / Vortex)",
        content_type = "multipart/form-data"
    ),
    responses(
        (status = 200, description = "File loaded into the warehouse", body = serde_json::Value),
        (status = 400, description = "Invalid file, schema or table name", body = ErrorResponse),
        (status = 413, description = "File exceeds the size limit", body = ErrorResponse),
        (status = 500, description = "Parse, upload or warehouse error", body = ErrorResponse)
    )
)]
pub async fn upload_file(
    State(state): State<AppState>,
    Query(params): Query<UploadParams>,
    multipart: Multipart,
) -> Result<Response, ApiError> {
    // Bound concurrent uploads to keep peak memory and warehouse load capped.
    // We take the permit *before* reading the body, so a shed request never
    // buffers it; the guard is held for the whole handler (through the COPY), so
    // it caps both the in-memory decode and concurrent loads. We shed rather
    // than queue: a 503 tells the client to retry instead of piling up here.
    let _permit = state
        .upload_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::Unavailable("Server at capacity, retry later".into()))?;

    let UploadedFile { file_name, data } = extract_file(multipart).await?;

    // The destination table is required: there is no silent fallback to the
    // file name, which is easy to get wrong (it would write to a table named
    // after whatever the client happened to call the file).
    let table_name = params
        .table
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::BadRequest("Missing required query parameter: table".into()))?;

    // Validate the table name up front — it is the trust boundary for the
    // identifier that flows into DDL/COPY and the object key. SQL identifiers
    // can't be bound parameters in any dialect, so we validate against a strict
    // allowlist rather than escape (see `database::Identifier`).
    let table = Identifier::new(table_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid table name {table_name:?}: {e}")))?;

    info!(file_name = %file_name, size = data.len(), table = %table, "Processing file upload");

    let schema_key = params.schema.clone().unwrap_or_else(|| "stage".to_string());

    // Data Vault load metadata, stamped once per upload so every row of this
    // batch shares it. `load_id` ties the batch together; `record_source` records
    // provenance; `load_date` (microseconds UTC) is the load timestamp. They are
    // only written for a DV-enriched schema (one that declares a business key).
    let record_source = format!("tako/{schema_key}/{table}");
    let load_id = uuid::Uuid::new_v4().to_string();
    let load_micros = chrono::Utc::now().timestamp_micros();

    // Parse, schema-validate, DV-enrich and Parquet-encode are all CPU-bound and
    // operate on the same DataFrame in sequence, so run them as one unit on a
    // blocking thread rather than on an async worker. `data` is moved in and
    // dropped at the end of the closure, so the source bytes do not outlive it.
    let registry = state.schema_registry.clone();
    let parquet_bytes = {
        let file_name = file_name.clone();
        let schema_key = schema_key.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>, ApiError> {
            // Resolve the schema once, then reuse it for validation and enrichment.
            let schema = registry.get_or_load(&schema_key).map_err(|e| match e {
                SchemaError::NotFound(k) => ApiError::BadRequest(format!("Schema not found: {k}")),
                other => ApiError::BadRequest(other.to_string()),
            })?;

            let df = parse::parse_data(&data, &file_name)
                .map_err(|e| ApiError::BadRequest(format!("Failed to parse file: {e}")))?;
            let df = validate_and_reorder(&schema, df)?;
            // No-op unless the schema declares a business key.
            let df = enrich_dataframe(df, &schema, &record_source, &load_id, load_micros)
                .map_err(|e| ApiError::internal("Data Vault enrichment failed", e))?;
            to_parquet(df)
        })
        .await
        .map_err(|e| ApiError::internal("Task panicked during file processing", e))??
    };

    let s3_path = format!("datascience/{table}.parquet");

    let upload_key = build_upload_key(&state.storage_config, &s3_path);

    info!(
        file_name = %file_name,
        upload_key = %upload_key,
        parquet_size = parquet_bytes.len(),
        "Uploading to blob storage"
    );

    state
        .file_client
        .upload_large_object(&upload_key, parquet_bytes.into())
        .await
        .map_err(|e| ApiError::internal("Failed to upload file to S3", e))?;

    info!(file_name = %file_name, "File uploaded successfully to S3");

    let copy_sql = load_into_warehouse(&state, &table, &s3_path).await?;

    info!(file_name = %file_name, copy_command = %copy_sql, "Copied data into warehouse");

    // Processing is fully synchronous: by this point parse, S3 upload and the
    // warehouse COPY have all completed. Report 200 OK / "done" so clients do
    // not poll for or retry a load that already finished (a retry would re-run
    // the append-only COPY and duplicate the rows).
    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "File uploaded and loaded into the warehouse",
            "file": file_name,
            "table": table.as_str(),
            "schema": schema_key,
            "status": "done",
        })),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_copy_path_local() {
        let config = StorageConfig::Local {
            root: "/data/mock_s3".to_string(),
        };
        assert_eq!(
            build_copy_path(&config, "datascience/test.parquet"),
            "/data/mock_s3/datascience/test.parquet"
        );
    }

    #[test]
    fn build_copy_path_s3() {
        let config = StorageConfig::S3 {
            data_lake: "example-datalake".to_string(),
            bucket: "my-bucket".to_string(),
        };
        assert_eq!(
            build_copy_path(&config, "datascience/test.parquet"),
            "s3://example-datalake/my-bucket/datascience/test.parquet"
        );
    }

    #[test]
    fn build_upload_key_local_passes_through() {
        let config = StorageConfig::Local {
            root: "/data/mock_s3".to_string(),
        };
        assert_eq!(
            build_upload_key(&config, "datascience/test.parquet"),
            "datascience/test.parquet"
        );
    }

    #[test]
    fn build_upload_key_s3_prefixes_bucket() {
        let config = StorageConfig::S3 {
            data_lake: "example-datalake".to_string(),
            bucket: "my-bucket".to_string(),
        };
        assert_eq!(
            build_upload_key(&config, "datascience/test.parquet"),
            "my-bucket/datascience/test.parquet"
        );
    }

    #[test]
    fn upload_key_and_copy_path_are_consistent_s3() {
        let config = StorageConfig::S3 {
            data_lake: "example-datalake".to_string(),
            bucket: "my-bucket".to_string(),
        };
        let key = "datascience/test.parquet";
        let upload_key = build_upload_key(&config, key);
        let copy_path = build_copy_path(&config, key);
        // The copy path should be s3://{data_lake}/{upload_key}
        assert_eq!(copy_path, format!("s3://example-datalake/{upload_key}"));
    }

    #[test]
    fn build_copy_path_s3_no_double_slash() {
        let config = StorageConfig::S3 {
            data_lake: "lake".to_string(),
            bucket: "bucket".to_string(),
        };
        let path = build_copy_path(&config, "key.parquet");
        assert!(
            !path.contains("//bucket"),
            "path should not contain double slashes: {path}"
        );
    }
}
