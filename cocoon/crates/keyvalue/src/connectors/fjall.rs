use fjall::{Database, Keyspace, KeyspaceCreateOptions, PersistMode};

use crate::traits::{KVError, KVResult, KeyValueStore};

/// Configuration for a Fjall-backed key-value store.
#[derive(Debug, Clone)]
pub struct FjallConfig {
    pub path: String,
    pub keyspace: String,
}

impl FjallConfig {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            keyspace: "default".to_string(),
        }
    }

    pub fn with_keyspace(mut self, keyspace: impl Into<String>) -> Self {
        self.keyspace = keyspace.into();
        self
    }
}

/// Fjall-backed key-value store (embedded LSM-tree).
pub struct FjallStore {
    db: Database,
    keyspace: Keyspace,
}

impl FjallStore {
    /// Open a Fjall store at the given path with default settings.
    pub fn connect(path: impl Into<String>) -> KVResult<Self> {
        Self::connect_with_config(FjallConfig::new(path))
    }

    /// Open a Fjall store from explicit configuration.
    pub fn connect_with_config(config: FjallConfig) -> KVResult<Self> {
        let db = Database::builder(&config.path)
            .open()
            .map_err(|e| KVError::ConnectionError(e.to_string()))?;

        let keyspace = db
            .keyspace(&config.keyspace, KeyspaceCreateOptions::default)
            .map_err(|e| KVError::ConnectionError(e.to_string()))?;

        Ok(Self { db, keyspace })
    }

    /// Flush data to disk for durability.
    pub fn persist(&self) -> KVResult<()> {
        self.db
            .persist(PersistMode::SyncAll)
            .map_err(|e| KVError::OperationError(e.to_string()))
    }
}

impl KeyValueStore for FjallStore {
    fn insert(&self, key: &str, value: &[u8]) -> KVResult<()> {
        self.keyspace
            .insert(key, value)
            .map_err(|e| KVError::OperationError(e.to_string()))
    }

    fn get(&self, key: &str) -> KVResult<Option<Vec<u8>>> {
        self.keyspace
            .get(key)
            .map(|opt| opt.map(|v| v.to_vec()))
            .map_err(|e| KVError::OperationError(e.to_string()))
    }

    fn remove(&self, key: &str) -> KVResult<()> {
        self.keyspace
            .remove(key)
            .map_err(|e| KVError::OperationError(e.to_string()))
    }

    fn contains_key(&self, key: &str) -> KVResult<bool> {
        self.keyspace
            .contains_key(key)
            .map_err(|e| KVError::OperationError(e.to_string()))
    }

    fn prefix(&self, prefix: &str) -> KVResult<Vec<(String, Vec<u8>)>> {
        let mut results = Vec::new();
        for guard in self.keyspace.prefix(prefix) {
            let (key, value) = guard
                .into_inner()
                .map_err(|e| KVError::OperationError(e.to_string()))?;
            results.push((String::from_utf8_lossy(&key).to_string(), value.to_vec()));
        }
        Ok(results)
    }

    fn range(&self, start: &str, end: &str) -> KVResult<Vec<(String, Vec<u8>)>> {
        let mut results = Vec::new();
        for guard in self.keyspace.range(start.as_bytes()..end.as_bytes()) {
            let (key, value) = guard
                .into_inner()
                .map_err(|e| KVError::OperationError(e.to_string()))?;
            results.push((String::from_utf8_lossy(&key).to_string(), value.to_vec()));
        }
        Ok(results)
    }
}
