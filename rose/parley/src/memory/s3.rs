//! MinIO / S3 memory store — the shared, durable backend.
//!
//! MinIO is a single-binary, S3-compatible object store. Talking to it through
//! the S3 API (not a bespoke format) means the same code reaches MinIO on a laptop
//! today and AWS S3 later, changing only endpoint and credentials.
//!
//! Chosen whenever `PARLEY_S3_ENDPOINT` is set (see `state.rs`). One object per
//! learner, `<learner>.json`, read whole and written whole — object storage's
//! natural grain for small per-learner blobs. See `docs/architecture/learner-state.md`.

use async_trait::async_trait;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::{Client, Config};
use tokio::sync::OnceCell;

use super::{Memories, MemoryStore};
use crate::domain::Level;

/// Connection settings, read from the environment in `state.rs`.
pub struct S3Config {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

/// Stores each learner's memories as an object keyed `<learner>.json`.
pub struct S3MemoryStore {
    client: Client,
    bucket: String,
    default_level: Level,
    /// The bucket is created lazily on first write, exactly once.
    bucket_ready: OnceCell<()>,
}

impl S3MemoryStore {
    /// Build the client. Path-style addressing is forced because MinIO requires it
    /// (virtual-host style assumes `bucket.host`, which MinIO does not serve).
    pub fn new(cfg: S3Config, default_level: Level) -> Self {
        let creds = Credentials::new(cfg.access_key, cfg.secret_key, None, None, "parley");
        let conf = Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(cfg.region))
            .endpoint_url(cfg.endpoint)
            .credentials_provider(creds)
            .force_path_style(true)
            .build();

        Self {
            client: Client::from_conf(conf),
            bucket: cfg.bucket,
            default_level,
            bucket_ready: OnceCell::new(),
        }
    }

    fn key_for(learner: &str) -> String {
        format!("{learner}.json")
    }

    /// Create the bucket if it does not exist. An already-owned bucket is success,
    /// not an error — this makes first run on a fresh MinIO just work.
    async fn ensure_bucket(&self) -> anyhow::Result<()> {
        self.bucket_ready
            .get_or_try_init(|| async {
                match self.client.create_bucket().bucket(&self.bucket).send().await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        let service = e.into_service_error();
                        // Bucket already there (owned by us, or just exists) → fine.
                        if service.is_bucket_already_owned_by_you()
                            || service.is_bucket_already_exists()
                        {
                            Ok(())
                        } else {
                            Err(anyhow::Error::from(service))
                        }
                    }
                }
            })
            .await
            .map(|_| ())
    }
}

#[async_trait]
impl MemoryStore for S3MemoryStore {
    async fn load(&self, learner: &str) -> anyhow::Result<Memories> {
        let key = Self::key_for(learner);
        let result = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await;

        match result {
            Ok(output) => {
                let bytes = output.body.collect().await?.into_bytes();
                Ok(serde_json::from_slice(&bytes)?)
            }
            Err(e) => {
                let service = e.into_service_error();
                // No object yet is not an error — a learner we have never seen.
                if service.is_no_such_key() {
                    Ok(Memories::empty(learner, self.default_level))
                } else {
                    Err(anyhow::Error::from(service))
                }
            }
        }
    }

    async fn save(&self, memories: &Memories) -> anyhow::Result<()> {
        self.ensure_bucket().await?;
        let key = Self::key_for(&memories.learner);
        let bytes = serde_json::to_vec_pretty(memories)?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(bytes))
            .content_type("application/json")
            .send()
            .await?;
        Ok(())
    }
}
