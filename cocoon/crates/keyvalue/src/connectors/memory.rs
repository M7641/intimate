use std::collections::BTreeMap;
use std::sync::RwLock;

use crate::traits::{KVResult, KeyValueStore};

/// In-memory key-value store backed by a `BTreeMap`.
///
/// Keys are kept in sorted order, so `prefix()` and `range()` queries
/// use efficient ordered iteration rather than full scans.
pub struct MemoryStore {
    data: RwLock<BTreeMap<String, Vec<u8>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(BTreeMap::new()),
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyValueStore for MemoryStore {
    fn insert(&self, key: &str, value: &[u8]) -> KVResult<()> {
        let mut data = self.data.write().unwrap();
        data.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &str) -> KVResult<Option<Vec<u8>>> {
        let data = self.data.read().unwrap();
        Ok(data.get(key).cloned())
    }

    fn remove(&self, key: &str) -> KVResult<()> {
        let mut data = self.data.write().unwrap();
        data.remove(key);
        Ok(())
    }

    fn contains_key(&self, key: &str) -> KVResult<bool> {
        let data = self.data.read().unwrap();
        Ok(data.contains_key(key))
    }

    fn prefix(&self, prefix: &str) -> KVResult<Vec<(String, Vec<u8>)>> {
        let data = self.data.read().unwrap();
        Ok(data
            .range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

    fn range(&self, start: &str, end: &str) -> KVResult<Vec<(String, Vec<u8>)>> {
        let data = self.data.read().unwrap();
        Ok(data
            .range(start.to_string()..end.to_string())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}
