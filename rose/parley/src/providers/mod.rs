//! The three capabilities of the voice loop, each behind a trait.
//!
//! Trait-per-capability is what lets us start with stubs and wire each model
//! independently. A handler depends on `dyn Conversant`, not on Ollama — so
//! swapping Ollama for something else never touches the routes.

use async_trait::async_trait;

use crate::domain::{Definition, Turn};

pub mod anthropic;
pub mod candle_llm;
pub mod candle_whisper;
pub mod ollama;
pub mod piper;
pub mod stub;
pub mod wiktionary;

/// Anything that can fail in a provider. Kept as a boxed error so each provider
/// can use its own error types (reqwest, io, etc.) without a shared enum.
pub type ProviderError = anyhow::Error;
pub type ProviderResult<T> = Result<T, ProviderError>;

/// Speech in the target language → text. (Whisper, in-process via Candle)
#[async_trait]
pub trait Transcriber: Send + Sync {
    /// `audio` is the raw bytes as captured by the browser; `mime` is its content
    /// type (e.g. "audio/webm"). Returns the recognised text.
    async fn transcribe(&self, audio: &[u8], mime: &str) -> ProviderResult<String>;
}

/// Conversation history → the tutor's next reply. (Mistral via Ollama)
#[async_trait]
pub trait Conversant: Send + Sync {
    /// `system` is the pedagogy prompt; `history` is the conversation so far,
    /// oldest first. Returns the tutor's reply text.
    async fn reply(&self, system: &str, history: &[Turn]) -> ProviderResult<String>;
}

/// Text → spoken audio in the target language. (Piper)
#[async_trait]
pub trait Synthesizer: Send + Sync {
    /// Returns audio bytes (WAV) for `text`.
    async fn synthesize(&self, text: &str) -> ProviderResult<Vec<u8>>;
}

/// A word → its definition, from a trusted dictionary. (Wiktionnaire)
#[async_trait]
pub trait Dictionary: Send + Sync {
    /// Look up `word` and return its senses. Errors when the word is not found or
    /// the source is unreachable — the caller decides whether that is fatal.
    async fn define(&self, word: &str) -> ProviderResult<Definition>;
}
