//! Local-filesystem memory store — the default, zero-server backend.
//!
//! One JSON file per learner under a data directory. This is what honours parley's
//! local-first thesis: memories persist across restarts with nothing to run and
//! nothing to configure. Chosen whenever no S3 endpoint is set (see `state.rs`).

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::{Memories, MemoryStore};
use crate::domain::Level;

/// Stores each learner's memories as `<dir>/<learner>.json`.
pub struct FsMemoryStore {
    dir: PathBuf,
    default_level: Level,
}

impl FsMemoryStore {
    /// Create the store rooted at `dir`, creating the directory if needed.
    pub fn new(dir: impl AsRef<Path>, default_level: Level) -> anyhow::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir, default_level })
    }

    fn path_for(&self, learner: &str) -> PathBuf {
        self.dir.join(format!("{learner}.json"))
    }
}

#[async_trait]
impl MemoryStore for FsMemoryStore {
    async fn load(&self, learner: &str) -> anyhow::Result<Memories> {
        let path = self.path_for(learner);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            // No file yet is not an error — a learner we have never seen.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Memories::empty(learner, self.default_level))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn save(&self, memories: &Memories) -> anyhow::Result<()> {
        let path = self.path_for(&memories.learner);
        let bytes = serde_json::to_vec_pretty(memories)?;
        std::fs::write(&path, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::DEFAULT_LEARNER;

    #[tokio::test]
    async fn missing_learner_loads_blank_then_round_trips() {
        let dir = std::env::temp_dir().join(format!("parley-mem-test-{}", std::process::id()));
        let store = FsMemoryStore::new(&dir, Level::Beginner).unwrap();

        let mut m = store.load(DEFAULT_LEARNER).await.unwrap();
        assert_eq!(m.distinct_word_count(), 0);

        m.record_words(["salut".into()]);
        store.save(&m).await.unwrap();

        let reloaded = store.load(DEFAULT_LEARNER).await.unwrap();
        assert_eq!(reloaded.distinct_word_count(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
