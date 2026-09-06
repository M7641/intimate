/// Metadata for a stored file/object
#[derive(Debug, Clone)]
pub struct FileMetadata {
    /// Size of the file in bytes
    pub content_length: i64,
    /// MIME type of the file
    pub content_type: Option<String>,
    /// Last modification timestamp
    pub last_modified: Option<String>,
    /// ETag or checksum of the file
    pub e_tag: Option<String>,
}

impl FileMetadata {
    /// Create a new FileMetadata instance
    pub fn new(
        content_length: i64,
        content_type: Option<String>,
        last_modified: Option<String>,
        e_tag: Option<String>,
    ) -> Self {
        Self {
            content_length,
            content_type,
            last_modified,
            e_tag,
        }
    }
}
