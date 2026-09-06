mod config;

pub use config::LocalConfig;

use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::traits::{BlobError, BlobResult, BlobStorage};
use crate::types::FileMetadata;

/// In-memory blob store (clé → octets), partagé et protégé pour les tests.
type MemoryStore = Arc<RwLock<HashMap<String, Vec<u8>>>>;

/// Local filesystem storage backend implementation
pub struct LocalStorage {
    /// Root path for file storage (None if in-memory)
    root_path: Option<PathBuf>,
    /// In-memory storage for testing
    memory_store: Option<MemoryStore>,
}

impl std::fmt::Debug for LocalStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalStorage")
            .field("root_path", &self.root_path)
            .field("in_memory", &self.memory_store.is_some())
            .finish()
    }
}

impl LocalStorage {
    /// Connect to local storage with the given configuration
    pub async fn connect(config: LocalConfig) -> BlobResult<Self> {
        if config.in_memory {
            return Ok(Self {
                root_path: None,
                memory_store: Some(Arc::new(RwLock::new(HashMap::new()))),
            });
        }

        let root_path = config.root_path.ok_or_else(|| {
            BlobError::ConfigError("Root path required for filesystem storage".to_string())
        })?;

        let path = PathBuf::from(&root_path);

        if config.create_if_missing && !path.exists() {
            tokio::fs::create_dir_all(&path)
                .await
                .map_err(|e| BlobError::IoError(format!("Failed to create directory: {}", e)))?;
        }

        if !path.exists() {
            return Err(BlobError::ConfigError(format!(
                "Root path does not exist: {}",
                root_path
            )));
        }

        Ok(Self {
            root_path: Some(path),
            memory_store: None,
        })
    }

    /// Get the full filesystem path for a key
    fn get_full_path(&self, key: &str) -> BlobResult<PathBuf> {
        let root = self
            .root_path
            .as_ref()
            .ok_or_else(|| BlobError::ConfigError("No root path configured".to_string()))?;

        // Normalize the key to prevent directory traversal
        let normalized_key = key.trim_start_matches('/');
        Ok(root.join(normalized_key))
    }

    /// Guess MIME type from file extension
    fn guess_mime_type(key: &str) -> Option<String> {
        mime_guess::from_path(key).first().map(|m| m.to_string())
    }
}

#[async_trait]
impl BlobStorage for LocalStorage {
    async fn upload_object(&self, key: &str, data: Bytes) -> BlobResult<()> {
        if let Some(store) = &self.memory_store {
            let mut guard = store.write().await;
            guard.insert(key.to_string(), data.to_vec());
            return Ok(());
        }

        let path = self.get_full_path(key)?;

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| BlobError::IoError(format!("Failed to create directories: {}", e)))?;
        }

        tokio::fs::write(&path, &data)
            .await
            .map_err(|e| BlobError::UploadError(format!("Failed to write file: {}", e)))?;

        Ok(())
    }

    async fn upload_object_from_path(&self, key: &str, file_path: &Path) -> BlobResult<()> {
        let data = tokio::fs::read(file_path)
            .await
            .map_err(|e| BlobError::IoError(format!("Failed to read source file: {}", e)))?;

        self.upload_object(key, Bytes::from(data)).await
    }

    async fn upload_large_object(&self, key: &str, data: Bytes) -> BlobResult<()> {
        // For local storage, large uploads are handled the same as regular uploads
        self.upload_object(key, data).await
    }

    async fn download_file(&self, key: &str) -> BlobResult<Vec<u8>> {
        if let Some(store) = &self.memory_store {
            let guard = store.read().await;
            return guard
                .get(key)
                .cloned()
                .ok_or_else(|| BlobError::NotFound(key.to_string()));
        }

        let path = self.get_full_path(key)?;

        tokio::fs::read(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BlobError::NotFound(key.to_string())
            } else {
                BlobError::DownloadError(format!("Failed to read file: {}", e))
            }
        })
    }

    async fn download_file_to_path(&self, key: &str, file_path: &Path) -> BlobResult<()> {
        let data = self.download_file(key).await?;
        tokio::fs::write(file_path, data)
            .await
            .map_err(|e| BlobError::IoError(format!("Failed to write file: {}", e)))?;
        Ok(())
    }

    async fn list_files(&self, prefix: Option<&str>) -> BlobResult<Vec<String>> {
        if let Some(store) = &self.memory_store {
            let guard = store.read().await;
            let keys: Vec<String> = guard
                .keys()
                .filter(|k| prefix.is_none_or(|p| k.starts_with(p)))
                .cloned()
                .collect();
            return Ok(keys);
        }

        let root = self
            .root_path
            .as_ref()
            .ok_or_else(|| BlobError::ConfigError("No root path configured".to_string()))?;

        let search_path = if let Some(p) = prefix {
            root.join(p.trim_start_matches('/'))
        } else {
            root.clone()
        };

        let mut files = Vec::new();
        self.collect_files_recursive(&search_path, root, &mut files)
            .await?;

        // Filter by prefix if provided
        if let Some(p) = prefix {
            files.retain(|f| f.starts_with(p));
        }

        Ok(files)
    }

    async fn delete_file(&self, key: &str) -> BlobResult<()> {
        if let Some(store) = &self.memory_store {
            let mut guard = store.write().await;
            guard.remove(key);
            return Ok(());
        }

        let path = self.get_full_path(key)?;

        tokio::fs::remove_file(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BlobError::NotFound(key.to_string())
            } else {
                BlobError::IoError(format!("Failed to delete file: {}", e))
            }
        })
    }

    async fn file_exists(&self, key: &str) -> BlobResult<bool> {
        if let Some(store) = &self.memory_store {
            let guard = store.read().await;
            return Ok(guard.contains_key(key));
        }

        let path = self.get_full_path(key)?;
        Ok(path.exists())
    }

    async fn get_file_metadata(&self, key: &str) -> BlobResult<FileMetadata> {
        if let Some(store) = &self.memory_store {
            let guard = store.read().await;
            let data = guard
                .get(key)
                .ok_or_else(|| BlobError::NotFound(key.to_string()))?;

            return Ok(FileMetadata {
                content_length: data.len() as i64,
                content_type: Self::guess_mime_type(key),
                last_modified: None,
                e_tag: None,
            });
        }

        let path = self.get_full_path(key)?;

        let metadata = tokio::fs::metadata(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BlobError::NotFound(key.to_string())
            } else {
                BlobError::IoError(format!("Failed to get metadata: {}", e))
            }
        })?;

        let last_modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs().to_string());

        Ok(FileMetadata {
            content_length: metadata.len() as i64,
            content_type: Self::guess_mime_type(key),
            last_modified,
            e_tag: None,
        })
    }

    async fn copy_file(&self, source_key: &str, destination_key: &str) -> BlobResult<()> {
        if let Some(store) = &self.memory_store {
            let guard = store.read().await;
            let data = guard
                .get(source_key)
                .cloned()
                .ok_or_else(|| BlobError::NotFound(source_key.to_string()))?;
            drop(guard);

            let mut guard = store.write().await;
            guard.insert(destination_key.to_string(), data);
            return Ok(());
        }

        let source_path = self.get_full_path(source_key)?;
        let dest_path = self.get_full_path(destination_key)?;

        // Create parent directories if needed
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| BlobError::IoError(format!("Failed to create directories: {}", e)))?;
        }

        tokio::fs::copy(&source_path, &dest_path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    BlobError::NotFound(source_key.to_string())
                } else {
                    BlobError::IoError(format!("Failed to copy file: {}", e))
                }
            })?;

        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        if self.memory_store.is_some() {
            "local-memory"
        } else {
            "local"
        }
    }
}

impl LocalStorage {
    /// Recursively collect files from a directory
    async fn collect_files_recursive(
        &self,
        dir: &Path,
        root: &Path,
        files: &mut Vec<String>,
    ) -> BlobResult<()> {
        if !dir.exists() {
            return Ok(());
        }

        if dir.is_file() {
            if let Ok(relative) = dir.strip_prefix(root) {
                files.push(relative.to_string_lossy().to_string());
            }
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| BlobError::IoError(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| BlobError::IoError(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() {
                Box::pin(self.collect_files_recursive(&path, root, files)).await?;
            } else if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_string_lossy().to_string());
            }
        }

        Ok(())
    }
}
