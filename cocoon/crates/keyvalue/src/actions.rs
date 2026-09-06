use crate::traits::{KVResult, KeyValueStore};

#[cfg(feature = "fjall")]
use crate::connectors::fjall::{FjallConfig, FjallStore};
#[cfg(feature = "memory")]
use crate::connectors::memory::MemoryStore;

// -- KVConfig --

#[derive(Debug, Clone)]
pub enum KVConfig {
    #[cfg(feature = "memory")]
    Memory,
    #[cfg(feature = "fjall")]
    Fjall(FjallConfig),
}

// -- KVType --

pub enum KVType {
    #[cfg(feature = "memory")]
    Memory,
    #[cfg(feature = "fjall")]
    Fjall,
}

// -- KVActions --

/// A synchronous key-value store wrapper that provides a consistent API
/// across different backends. Uses dynamic dispatch via `Box<dyn KeyValueStore>`.
pub struct KVActions {
    kv_type: KVType,
    store: Box<dyn KeyValueStore>,
}

impl KVActions {
    /// Create a new key-value store from explicit configuration.
    pub fn from_config(config: KVConfig) -> KVResult<Self> {
        match config {
            #[cfg(feature = "memory")]
            KVConfig::Memory => Ok(Self {
                kv_type: KVType::Memory,
                store: Box::new(MemoryStore::new()),
            }),
            #[cfg(feature = "fjall")]
            KVConfig::Fjall(cfg) => {
                let store = FjallStore::connect_with_config(cfg)?;
                Ok(Self {
                    kv_type: KVType::Fjall,
                    store: Box::new(store),
                })
            }
        }
    }

    pub fn insert(&self, key: &str, value: &[u8]) -> KVResult<()> {
        self.store.insert(key, value)
    }

    pub fn get(&self, key: &str) -> KVResult<Option<Vec<u8>>> {
        self.store.get(key)
    }

    pub fn remove(&self, key: &str) -> KVResult<()> {
        self.store.remove(key)
    }

    pub fn contains_key(&self, key: &str) -> KVResult<bool> {
        self.store.contains_key(key)
    }

    pub fn prefix(&self, prefix: &str) -> KVResult<Vec<(String, Vec<u8>)>> {
        self.store.prefix(prefix)
    }

    pub fn range(&self, start: &str, end: &str) -> KVResult<Vec<(String, Vec<u8>)>> {
        self.store.range(start, end)
    }

    /// Get a reference to the underlying `KeyValueStore` trait object.
    pub fn as_store(&self) -> &dyn KeyValueStore {
        self.store.as_ref()
    }

    pub fn kv_type(&self) -> &KVType {
        &self.kv_type
    }
}
