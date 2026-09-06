//! Blob Storage Abstraction
//!
//! A unified interface for blob storage backends including AWS S3 and local filesystem.
//!
//! # Features
//!
//! - `s3` (default): AWS S3 storage backend
//! - `local`: Local filesystem storage backend
//! - `full`: Enable all backends
//!
//! # Example
//!
//! ```rust,no_run
//! use blobs::{new, StorageBackend};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Production (S3)
//!     let client = new(StorageBackend::s3("my-bucket".into())).await?;
//!
//!     // Development (local filesystem)
//!     // let client = new(StorageBackend::local("/tmp/blobs".into())).await?;
//!
//!     // Testing (in-memory)
//!     // let client = new(StorageBackend::local_memory()).await?;
//!
//!     // All have the same interface
//!     // client.upload_object("key", data).await?;
//!     Ok(())
//! }
//! ```

mod config;
mod connectors;
mod traits;
mod types;

pub use config::StorageBackend;
pub use traits::{BlobError, BlobResult, BlobStorage};
pub use types::FileMetadata;

#[cfg(feature = "s3")]
pub use connectors::s3::{S3Config, S3Storage};

#[cfg(feature = "local")]
pub use connectors::local::{LocalConfig, LocalStorage};

use std::sync::Arc;

/// Create a new blob storage client with the given backend configuration
///
/// # Arguments
///
/// * `backend` - The storage backend configuration
///
/// # Returns
///
/// An `Arc<dyn BlobStorage>` that can be used for blob operations
///
/// # Example
///
/// ```rust,no_run
/// use blobs::{new, StorageBackend};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create S3 client
/// let s3_client = new(StorageBackend::s3("my-bucket".into())).await?;
///
/// // Create local filesystem client
/// // let local_client = new(StorageBackend::local("/tmp/blobs".into())).await?;
///
/// // Create in-memory client for testing
/// // let memory_client = new(StorageBackend::local_memory()).await?;
/// # Ok(())
/// # }
/// ```
pub async fn new(backend: StorageBackend) -> BlobResult<Arc<dyn BlobStorage>> {
    match backend {
        #[cfg(feature = "s3")]
        StorageBackend::S3(config) => {
            let storage = connectors::s3::S3Storage::connect(config).await?;
            Ok(Arc::new(storage))
        }
        #[cfg(feature = "local")]
        StorageBackend::Local(config) => {
            let storage = connectors::local::LocalStorage::connect(config).await?;
            Ok(Arc::new(storage))
        }
    }
}

// Backward compatibility: re-export S3Client as alias when s3 feature is enabled
#[cfg(feature = "s3")]
pub type S3Client = S3Storage;
