/// Configuration for local filesystem storage backend
#[derive(Debug, Clone)]
pub struct LocalConfig {
    /// Root path for file storage
    pub root_path: Option<String>,
    /// Create root directory if it doesn't exist
    pub create_if_missing: bool,
    /// Use in-memory storage (for testing)
    pub in_memory: bool,
}

impl LocalConfig {
    /// Create a new LocalConfig with a root path
    pub fn new(root_path: String) -> Self {
        Self {
            root_path: Some(root_path),
            create_if_missing: true,
            in_memory: false,
        }
    }

    /// Create an in-memory storage configuration for testing
    pub fn in_memory() -> Self {
        Self {
            root_path: None,
            create_if_missing: false,
            in_memory: true,
        }
    }

    /// Set whether to create root directory if missing
    pub fn with_create_if_missing(mut self, create: bool) -> Self {
        self.create_if_missing = create;
        self
    }
}
