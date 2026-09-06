/// Configuration for S3 storage backend
#[derive(Debug, Clone)]
pub struct S3Config {
    /// S3 bucket name
    pub bucket: String,
    /// AWS region (defaults to us-east-1 if not specified)
    pub region: Option<String>,
    /// Custom endpoint URL (for S3-compatible services like MinIO)
    pub endpoint_url: Option<String>,
    /// Use path-style addressing (required for some S3-compatible services)
    pub path_style: bool,
}

impl S3Config {
    /// Create a new S3Config with default settings
    pub fn new(bucket: String) -> Self {
        Self {
            bucket,
            region: None,
            endpoint_url: None,
            path_style: false,
        }
    }

    /// Create an S3Config with a specific region
    pub fn with_region(bucket: String, region: String) -> Self {
        Self {
            bucket,
            region: Some(region),
            endpoint_url: None,
            path_style: false,
        }
    }

    /// Set a custom endpoint URL (for S3-compatible services)
    pub fn with_endpoint(mut self, endpoint_url: String) -> Self {
        self.endpoint_url = Some(endpoint_url);
        self
    }

    /// Enable path-style addressing
    pub fn with_path_style(mut self, path_style: bool) -> Self {
        self.path_style = path_style;
        self
    }
}
