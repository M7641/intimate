mod config;

pub use config::S3Config;

use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{
    Client,
    primitives::ByteStream,
    types::{CompletedMultipartUpload, CompletedPart},
};
use bytes::Bytes;
use std::path::Path;

use aws_sdk_s3::error::DisplayErrorContext;

use crate::traits::{BlobError, BlobResult, BlobStorage};
use crate::types::FileMetadata;

const CHUNK_SIZE: usize = 5 * 1024 * 1024; // 5MB chunks for multipart upload

/// Format an AWS SDK error with its full causal chain.
fn fmt_sdk_err(e: &dyn std::error::Error) -> String {
    DisplayErrorContext(e).to_string()
}

/// S3 storage backend implementation
#[derive(Debug)]
pub struct S3Storage {
    client: Client,
    bucket: String,
}

impl S3Storage {
    /// Connect to S3 with the given configuration
    pub async fn connect(config: S3Config) -> BlobResult<Self> {
        let region_provider = if let Some(region) = &config.region {
            RegionProviderChain::first_try(aws_config::Region::new(region.clone()))
        } else {
            RegionProviderChain::default_provider().or_else("us-east-1")
        };

        let mut aws_config_builder = aws_config::from_env().region(region_provider);

        if let Some(endpoint) = &config.endpoint_url {
            aws_config_builder = aws_config_builder.endpoint_url(endpoint);
        }

        let aws_config = aws_config_builder.load().await;

        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&aws_config);
        if config.path_style {
            s3_config_builder = s3_config_builder.force_path_style(true);
        }

        let client = Client::from_conf(s3_config_builder.build());

        Ok(Self {
            client,
            bucket: config.bucket,
        })
    }
}

#[async_trait]
impl BlobStorage for S3Storage {
    async fn upload_object(&self, key: &str, data: Bytes) -> BlobResult<()> {
        let body = ByteStream::from(data);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .send()
            .await
            .map_err(|e| BlobError::UploadError(fmt_sdk_err(&e)))?;

        Ok(())
    }

    async fn upload_object_from_path(&self, key: &str, file_path: &Path) -> BlobResult<()> {
        let body = ByteStream::from_path(file_path)
            .await
            .map_err(|e| BlobError::IoError(e.to_string()))?;

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .send()
            .await
            .map_err(|e| BlobError::UploadError(fmt_sdk_err(&e)))?;

        Ok(())
    }

    async fn upload_large_object(&self, key: &str, data: Bytes) -> BlobResult<()> {
        if data.len() < CHUNK_SIZE {
            return self.upload_object(key, data).await;
        }

        let multipart_upload = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| BlobError::UploadError(fmt_sdk_err(&e)))?;

        let upload_id = multipart_upload
            .upload_id()
            .ok_or_else(|| BlobError::UploadError("No upload ID returned".to_string()))?
            .to_string();

        let mut parts = Vec::new();
        let chunks: Vec<&[u8]> = data.chunks(CHUNK_SIZE).collect();

        for (i, chunk) in chunks.iter().enumerate() {
            let part_number = (i + 1) as i32;
            let body = ByteStream::from(Bytes::copy_from_slice(chunk));

            let upload_part = self
                .client
                .upload_part()
                .bucket(&self.bucket)
                .key(key)
                .upload_id(&upload_id)
                .part_number(part_number)
                .body(body)
                .send()
                .await
                .map_err(|e| BlobError::UploadError(fmt_sdk_err(&e)))?;

            parts.push(
                CompletedPart::builder()
                    .e_tag(upload_part.e_tag().unwrap_or_default())
                    .part_number(part_number)
                    .build(),
            );
        }

        let completed_upload = CompletedMultipartUpload::builder()
            .set_parts(Some(parts))
            .build();

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(&upload_id)
            .multipart_upload(completed_upload)
            .send()
            .await
            .map_err(|e| BlobError::UploadError(fmt_sdk_err(&e)))?;

        Ok(())
    }

    async fn download_file(&self, key: &str) -> BlobResult<Vec<u8>> {
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                let detail = fmt_sdk_err(&e);
                if detail.contains("404") || detail.contains("NoSuchKey") {
                    BlobError::NotFound(key.to_string())
                } else {
                    BlobError::DownloadError(detail)
                }
            })?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| BlobError::DownloadError(e.to_string()))?;

        Ok(data.into_bytes().to_vec())
    }

    async fn download_file_to_path(&self, key: &str, file_path: &Path) -> BlobResult<()> {
        let data = self.download_file(key).await?;
        tokio::fs::write(file_path, data)
            .await
            .map_err(|e| BlobError::IoError(e.to_string()))?;
        Ok(())
    }

    async fn list_files(&self, prefix: Option<&str>) -> BlobResult<Vec<String>> {
        let mut request = self.client.list_objects_v2().bucket(&self.bucket);

        if let Some(p) = prefix {
            request = request.prefix(p);
        }

        let response = request
            .send()
            .await
            .map_err(|e| BlobError::Other(fmt_sdk_err(&e)))?;

        let keys = response
            .contents()
            .iter()
            .filter_map(|obj| obj.key().map(|k| k.to_string()))
            .collect();

        Ok(keys)
    }

    async fn delete_file(&self, key: &str) -> BlobResult<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| BlobError::Other(fmt_sdk_err(&e)))?;

        Ok(())
    }

    async fn file_exists(&self, key: &str) -> BlobResult<bool> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let detail = fmt_sdk_err(&e);
                if detail.contains("404") || detail.contains("NotFound") {
                    Ok(false)
                } else {
                    Err(BlobError::Other(detail))
                }
            }
        }
    }

    async fn get_file_metadata(&self, key: &str) -> BlobResult<FileMetadata> {
        let response = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                let detail = fmt_sdk_err(&e);
                if detail.contains("404") || detail.contains("NotFound") {
                    BlobError::NotFound(key.to_string())
                } else {
                    BlobError::Other(detail)
                }
            })?;

        Ok(FileMetadata {
            content_length: response.content_length().unwrap_or(0),
            content_type: response.content_type().map(|s| s.to_string()),
            last_modified: response.last_modified().map(|dt| dt.to_string()),
            e_tag: response.e_tag().map(|s| s.to_string()),
        })
    }

    async fn copy_file(&self, source_key: &str, destination_key: &str) -> BlobResult<()> {
        let copy_source = format!("{}/{}", self.bucket, source_key);

        self.client
            .copy_object()
            .bucket(&self.bucket)
            .copy_source(copy_source)
            .key(destination_key)
            .send()
            .await
            .map_err(|e| BlobError::Other(fmt_sdk_err(&e)))?;

        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "s3"
    }
}
