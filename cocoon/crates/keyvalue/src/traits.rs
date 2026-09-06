/// Errors that can occur during key-value operations.
#[derive(Debug)]
pub enum KVError {
    ConnectionError(String),
    OperationError(String),
    NotFound,
    SerializationError(String),
    Other(String),
}

impl std::fmt::Display for KVError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KVError::ConnectionError(msg) => write!(f, "Connection error: {msg}"),
            KVError::OperationError(msg) => write!(f, "Operation error: {msg}"),
            KVError::NotFound => write!(f, "Key not found"),
            KVError::SerializationError(msg) => write!(f, "Serialization error: {msg}"),
            KVError::Other(msg) => write!(f, "KV error: {msg}"),
        }
    }
}

impl std::error::Error for KVError {}

pub type KVResult<T> = Result<T, KVError>;

/// Core key-value store trait — sync, object-safe, thread-safe.
pub trait KeyValueStore: Send + Sync {
    /// Insert or overwrite a key-value pair.
    fn insert(&self, key: &str, value: &[u8]) -> KVResult<()>;

    /// Get the value for a key, or `None` if it doesn't exist.
    fn get(&self, key: &str) -> KVResult<Option<Vec<u8>>>;

    /// Remove a key-value pair. No-op if the key doesn't exist.
    fn remove(&self, key: &str) -> KVResult<()>;

    /// Check whether a key exists.
    fn contains_key(&self, key: &str) -> KVResult<bool>;

    /// Return all key-value pairs whose key starts with `prefix`, sorted by key.
    fn prefix(&self, prefix: &str) -> KVResult<Vec<(String, Vec<u8>)>>;

    /// Return all key-value pairs whose key is in `[start, end)`, sorted by key.
    fn range(&self, start: &str, end: &str) -> KVResult<Vec<(String, Vec<u8>)>>;
}
