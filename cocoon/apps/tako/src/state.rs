use blobs::{BlobStorage, StorageBackend, new};
use database::ApiDbActions;
use schema::SchemaRegistry;
use service_kit::RateLimiter;
use service_kit::metrics::PrometheusHandle;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::info;

use crate::limits;

/// Resolve a runtime data directory.
///
/// Prefers `env_var` — that is how real deployments point at their schemas and
/// storage. Otherwise it falls back to `<workspace>/<dev_subdir>`, derived from
/// `CARGO_MANIFEST_DIR`. That constant is resolved at **compile time**, so the
/// fallback is only meaningful when running from the source tree (`cargo run` in
/// development); it will not exist in a container, where `env_var` is expected
/// to be set.
fn resolve_data_dir(env_var: &str, dev_subdir: &str) -> PathBuf {
    match std::env::var_os(env_var) {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(dev_subdir),
    }
}

#[derive(Debug, Clone)]
pub enum StorageConfig {
    Local { root: String },
    S3 { data_lake: String, bucket: String },
}

#[derive(Clone)]
pub struct AppState {
    pub file_client: Arc<dyn BlobStorage>,
    pub storage_config: StorageConfig,
    pub db: ApiDbActions,
    pub schema_registry: Arc<SchemaRegistry>,
    /// Bounds concurrent `/upload` processing so peak memory and warehouse load
    /// stay capped; the handler sheds (503) when no permit is available.
    pub upload_semaphore: Arc<Semaphore>,
    /// Renders Prometheus metrics for `/metrics`. The recorder it reads from is
    /// installed once in [`AppState::new`]; `middleware::metrics_layer` feeds it.
    pub metrics_handle: PrometheusHandle,
    /// Coarse global request-rate backstop for the load-bearing routes (a fixed
    /// per-second window). Per-client fairness is the gateway's job; this only
    /// caps total throughput defensively.
    pub rate_limiter: RateLimiter,
}

impl AppState {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let _ = dotenvy::dotenv();
        let tenant = std::env::var("TENANT").unwrap_or_else(|_| "tenant".to_string());
        let backend = std::env::var("DATA_WAREHOUSE_TYPE").unwrap_or_else(|_| "duckdb".to_string());

        // Flex the warehouse pool and statement timeout to the deployment
        // (DB_MAX_CONNECTIONS / DB_STATEMENT_TIMEOUT_SECS). The timeout is a
        // no-op for the in-process DuckDB backend.
        let pool_size = limits::db_pool_size() as u32;
        let statement_timeout = limits::db_statement_timeout();

        let (file_client, storage_config, db) = match backend.as_str() {
            "duckdb" => {
                let root = resolve_data_dir("LOCAL_STORAGE_ROOT", "mock_s3")
                    .to_string_lossy()
                    .into_owned();
                let client = new(StorageBackend::local(root.clone())).await?;
                let db = ApiDbActions::connect_with_timeout(
                    "duckdb",
                    Some(pool_size),
                    statement_timeout,
                )?;
                (client, StorageConfig::Local { root }, db)
            }
            "postgres" | "amazon_redshift" | "snowflake" => {
                let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| tenant.clone());
                // Required: never default the data-lake bucket. A wrong default
                // would write a customer's data into the wrong place, so fail
                // startup loudly instead.
                let data_lake = std::env::var("DATA_LAKE").map_err(|_| {
                    format!(
                        "DATA_LAKE must be set for the {backend:?} backend \
                         (the S3 data-lake bucket has no safe default)"
                    )
                })?;
                let client = new(StorageBackend::s3(data_lake.clone())).await?;
                info!(data_lake = %data_lake, bucket = %bucket, "Initialized S3 storage client");
                let config = StorageConfig::S3 { data_lake, bucket };
                let db = ApiDbActions::connect_with_timeout(
                    &backend,
                    Some(pool_size),
                    statement_timeout,
                )?;
                (client, config, db)
            }
            other => return Err(format!("Unknown DATA_WAREHOUSE_TYPE: {other}").into()),
        };

        let schema_dir = resolve_data_dir("SCHEMA_DIR", "schemas");
        info!(schema_dir = %schema_dir.display(), "Loading schema registry");

        // Install the global Prometheus recorder once; the handle renders it at
        // `/metrics`, and `middleware::metrics_layer` records request metrics into it.
        let metrics_handle = service_kit::metrics::install_recorder();

        let max_concurrent_uploads = limits::max_concurrent_uploads();

        // Fixed 1-second window: at most `rate_limit_rps` requests per second
        // across the load-bearing routes. Spawns a background reset task (we are
        // inside the tokio runtime here).
        let rate_limiter =
            RateLimiter::new(limits::rate_limit_rps() as u64, Duration::from_secs(1));

        Ok(Self {
            file_client,
            storage_config,
            db,
            schema_registry: Arc::new(SchemaRegistry::new(schema_dir)),
            upload_semaphore: Arc::new(Semaphore::new(max_concurrent_uploads)),
            metrics_handle,
            rate_limiter,
        })
    }
}
