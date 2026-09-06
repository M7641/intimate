#[cfg(feature = "s3")]
use crate::connectors::s3::S3Config;

#[cfg(feature = "local")]
use crate::connectors::local::LocalConfig;

/// Configuration enum for selecting storage backend
#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// AWS S3 storage backend
    #[cfg(feature = "s3")]
    S3(S3Config),
    /// Local filesystem storage backend
    #[cfg(feature = "local")]
    Local(LocalConfig),
}

impl StorageBackend {
    /// Create an S3 backend with default region
    #[cfg(feature = "s3")]
    pub fn s3(bucket: String) -> Self {
        StorageBackend::S3(S3Config::new(bucket))
    }

    /// Create an S3 backend with a specific region
    #[cfg(feature = "s3")]
    pub fn s3_with_region(bucket: String, region: String) -> Self {
        StorageBackend::S3(S3Config::with_region(bucket, region))
    }

    /// Create a local filesystem backend
    #[cfg(feature = "local")]
    pub fn local(root_path: String) -> Self {
        StorageBackend::Local(LocalConfig::new(root_path))
    }

    /// Create an in-memory backend for testing
    #[cfg(feature = "local")]
    pub fn local_memory() -> Self {
        StorageBackend::Local(LocalConfig::in_memory())
    }
}
