//! Durable memories about the learner — the learner-state spine.
//!
//! Chat history (`state.rs::Sessions`) is per-session and lives in RAM. Memories
//! are the opposite: few, durable, cross-session facts distilled from many
//! conversations — the compressed profile a good tutor keeps in their head.
//!
//! Persistence sits behind the `MemoryStore` trait, exactly like the three voice
//! capabilities in `providers/`. Handlers depend on `dyn MemoryStore`, never on a
//! concrete backend, so swapping local files for MinIO never touches the routes.
//!
//! See `docs/architecture/learner-state.md` for the full design.

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::{Definition, Level};

pub mod fs;
pub mod s3;

/// Milliseconds since the Unix epoch. Small dependency-free timestamp.
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// One word the learner has produced, with how often and when last seen. The seed
/// of both the Progress view and (later) the spaced-repetition schedule.
///
/// A word starts out merely *seen* (captured from a conversation turn). The learner
/// can then *save* it, which fetches its `definition` from a trusted dictionary and
/// keeps it for study. Saved and seen live in the same list so a saved word carries
/// its usage history with it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabItem {
    pub word: String,
    pub count: u32,
    pub last_seen: u64,
    /// Whether the learner has deliberately saved this word for study.
    #[serde(default)]
    pub saved: bool,
    /// The definition, once collected from the dictionary. `None` until saved (or
    /// if the lookup found nothing).
    #[serde(default)]
    pub definition: Option<Definition>,
}

/// Everything durable we know about one learner. Read whole, written whole — the
/// natural grain of an object store (see the MinIO backend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memories {
    pub learner: String,
    pub level: Level,
    /// Words the learner has actually produced, newest activity bubbling up.
    #[serde(default)]
    pub vocab: Vec<VocabItem>,
}

impl Memories {
    /// A blank profile — what `load` returns for a learner we have never seen.
    pub fn empty(learner: impl Into<String>, level: Level) -> Self {
        Self { learner: learner.into(), level, vocab: Vec::new() }
    }

    /// Fold newly-produced words into the vocabulary: bump the count and touch the
    /// timestamp for each. This is the deterministic write that keeps memories from
    /// being dead — no model call needed.
    pub fn record_words(&mut self, words: impl IntoIterator<Item = String>) {
        let now = now_millis();
        for word in words {
            match self.vocab.iter_mut().find(|v| v.word == word) {
                Some(existing) => {
                    existing.count += 1;
                    existing.last_seen = now;
                }
                None => self.vocab.push(VocabItem {
                    word,
                    count: 1,
                    last_seen: now,
                    saved: false,
                    definition: None,
                }),
            }
        }
    }

    /// Deliberately save a word for study, attaching its definition. Upserts: a word
    /// already in the vocabulary is marked saved and given its definition; a brand
    /// new word is added. Returns the resulting item. `last_seen` is touched so the
    /// word bubbles to the top of the saved list.
    pub fn save_word(&mut self, word: &str, definition: Option<Definition>) -> VocabItem {
        let now = now_millis();
        match self.vocab.iter_mut().find(|v| v.word == word) {
            Some(existing) => {
                existing.saved = true;
                existing.last_seen = now;
                // Only overwrite the definition when a fresh one was found, so a
                // failed re-lookup never wipes a definition we already had.
                if definition.is_some() {
                    existing.definition = definition;
                }
                existing.clone()
            }
            None => {
                let item = VocabItem {
                    word: word.to_string(),
                    count: 0,
                    last_seen: now,
                    saved: true,
                    definition,
                };
                self.vocab.push(item.clone());
                item
            }
        }
    }

    /// The words the learner has saved, most recently saved first.
    pub fn saved_words(&self) -> Vec<VocabItem> {
        let mut saved: Vec<VocabItem> = self.vocab.iter().filter(|v| v.saved).cloned().collect();
        saved.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        saved
    }

    /// Total distinct words the learner has produced.
    pub fn distinct_word_count(&self) -> usize {
        self.vocab.len()
    }
}

/// Where memories live. Two implementations: local files (default) and MinIO/S3.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Load a learner's memories, or a blank profile if we have never seen them.
    async fn load(&self, learner: &str) -> anyhow::Result<Memories>;

    /// Persist a learner's memories, overwriting any prior copy.
    async fn save(&self, memories: &Memories) -> anyhow::Result<()>;
}

/// The single-learner id used until multi-learner support lands. Centralised so
/// the "who is this" question has one answer to change later.
pub const DEFAULT_LEARNER: &str = "default";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_words_bumps_counts_and_dedupes() {
        let mut m = Memories::empty(DEFAULT_LEARNER, Level::Beginner);
        m.record_words(["bonjour".into(), "chat".into(), "bonjour".into()]);
        assert_eq!(m.distinct_word_count(), 2);
        let bonjour = m.vocab.iter().find(|v| v.word == "bonjour").unwrap();
        assert_eq!(bonjour.count, 2);
    }
}
