pub mod actions;
pub mod connectors;
pub mod traits;

// -- Re-exports --

pub use actions::{KVActions, KVConfig, KVType};
pub use traits::{KVError, KVResult, KeyValueStore};

#[cfg(feature = "fjall")]
pub use connectors::fjall::{FjallConfig, FjallStore};
#[cfg(feature = "memory")]
pub use connectors::memory::MemoryStore;
