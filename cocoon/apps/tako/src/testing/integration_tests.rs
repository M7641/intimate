//! End-to-end test of the HTTP surface against an in-process, file-backed
//! DuckDB — no network, no real warehouse. Drives the real router
//! (`create_router`) with `tower::ServiceExt::oneshot`, exercising the riskiest
//! path: create table → upload (parse → Parquet → local S3 → `COPY`) → inspect,
//! plus the identifier injection guard.
//!
//! DuckDB is file-backed (not `:memory:`) with a single pooled connection, so the
//! table created by `/table` is visible to the `/upload` `COPY`. Moving this kind
//! of coverage onto testcontainers is tracked in the workspace `README`.

use std::sync::Mutex;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // for `oneshot`

use crate::create_router;

/// Serialises tests that mutate process-global env (and the DuckDB file).
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// The committed schema directory (`schemas/sample_data.json` lives here).
fn workspace_schemas_dir() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas")
}

/// Build a minimal `multipart/form-data` body with a single `file` field.
fn multipart_body(boundary: &str, filename: &str, content: &str) -> String {
    format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
         Content-Type: text/csv\r\n\r\n\
         {content}\r\n\
         --{boundary}--\r\n"
    )
}

#[tokio::test]
async fn upload_pipeline_end_to_end_on_duckdb() {
    let _guard = ENV_LOCK.lock().unwrap();

    // Isolated temp workspace for this run.
    let base = std::env::temp_dir().join(format!("tako_itest_{}", std::process::id()));
    let storage = base.join("storage");
    std::fs::create_dir_all(storage.join("datascience")).unwrap();
    let db_path = base.join("itest.duckdb");

    // File-backed, single-connection DuckDB so the CREATE and the COPY share
    // state. SAFETY: tests holding `ENV_LOCK` are the only writers of these vars.
    unsafe {
        std::env::set_var("DATA_WAREHOUSE_TYPE", "duckdb");
        std::env::set_var("DUCKDB_PATH", &db_path);
        std::env::set_var("DB_MAX_CONNECTIONS", "1");
        std::env::set_var("LOCAL_STORAGE_ROOT", &storage);
        std::env::set_var("SCHEMA_DIR", workspace_schemas_dir());
    }

    let app = create_router().await.expect("router builds on duckdb");

    // 1. Create the staging table from the committed `sample_data` schema.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/table?table=people&schema=sample_data")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "create table");

    // 2. Upload a CSV matching the schema: parse → Parquet → local S3 → COPY.
    let boundary = "tako-itest-boundary";
    let csv = "id,name,age,score,active\n1,alice,30,9.5,true\n2,bob,40,8.1,false\n";
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/upload?table=people&schema=sample_data")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(multipart_body(boundary, "people.csv", csv)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "upload loads into the warehouse"
    );

    // 3. Inspect the table — the schema's columns should be reported.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/table/people")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "get table detail");
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    // The business columns, plus the Data Vault metadata added because the
    // `sample_data` schema declares a business key. Their presence proves both
    // the DV-aware DDL and that the enriched Parquet `COPY`ed cleanly.
    for col in [
        "id",
        "name",
        "age",
        "score",
        "active",
        "sample_data_hk",
        "hashdiff",
        "load_date",
        "record_source",
        "load_id",
    ] {
        assert!(body.contains(col), "table detail missing {col}: {body}");
    }

    // 4. Re-upload the same file. Data Vault is insert-only, so a repeated load is
    // accepted (200), not rejected — the rows are traceable (new load_id) and
    // deduplicable downstream by hashdiff, rather than a silent error.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/upload?table=people&schema=sample_data")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(multipart_body(boundary, "people.csv", csv)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "re-upload is accepted (insert-only)"
    );

    // 5. Injection guard: a non-identifier table name is rejected up front.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/table?table=bad%3Bdrop&schema=sample_data")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "injection in table name is rejected"
    );

    let _ = std::fs::remove_dir_all(&base);
}
