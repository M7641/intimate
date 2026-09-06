use async_trait::async_trait;
use bytes::Bytes;
use std::fmt;
use std::path::Path;

use crate::types::FileMetadata;

/// Error types for blob storage operations
#[derive(Debug)]
pub enum BlobError {
    /// Failed to establish connection to storage backend
    ConnectionError(String),
    /// Requested file/object not found
    NotFound(String),
    /// Permission denied for the requested operation
    PermissionDenied(String),
    /// I/O error during file operations
    IoError(String),
    /// Configuration error
    ConfigError(String),
    /// Failed to upload file/object
    UploadError(String),
    /// Failed to download file/object
    DownloadError(String),
    /// Other unspecified error
    Other(String),
}

impl fmt::Display for BlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlobError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            BlobError::NotFound(msg) => write!(f, "Not found: {}", msg),
            BlobError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            BlobError::IoError(msg) => write!(f, "I/O error: {}", msg),
            BlobError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            BlobError::UploadError(msg) => write!(f, "Upload error: {}", msg),
            BlobError::DownloadError(msg) => write!(f, "Download error: {}", msg),
            BlobError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for BlobError {}

/// Result type alias for blob operations
pub type BlobResult<T> = Result<T, BlobError>;

/// Abstract trait for blob storage backends
#[async_trait]
pub trait BlobStorage: Send + Sync {
    /// Upload data to storage with the given key
    async fn upload_object(&self, key: &str, data: Bytes) -> BlobResult<()>;

    /// Upload a file from a local path to storage
    async fn upload_object_from_path(&self, key: &str, file_path: &Path) -> BlobResult<()>;

    /// Upload large data using multipart upload (or equivalent)
    async fn upload_large_object(&self, key: &str, data: Bytes) -> BlobResult<()>;

    /// Download a file from storage
    async fn download_file(&self, key: &str) -> BlobResult<Vec<u8>>;

    /// Download a file from storage and save to local path
    async fn download_file_to_path(&self, key: &str, file_path: &Path) -> BlobResult<()>;

    /// List files in storage with optional prefix filter
    async fn list_files(&self, prefix: Option<&str>) -> BlobResult<Vec<String>>;

    /// Delete a file from storage
    async fn delete_file(&self, key: &str) -> BlobResult<()>;

    /// Check if a file exists in storage
    async fn file_exists(&self, key: &str) -> BlobResult<bool>;

    /// Get metadata for a file
    async fn get_file_metadata(&self, key: &str) -> BlobResult<FileMetadata>;

    /// Copy a file within storage
    async fn copy_file(&self, source_key: &str, destination_key: &str) -> BlobResult<()>;

    /// Get the name of the storage backend
    fn backend_name(&self) -> &'static str;
}
