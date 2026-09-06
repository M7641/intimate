//! No-op providers so the loop runs end-to-end before any model is installed.
//!
//! Each stub stands in for one capability and returns something harmless. Useful
//! for developing the frontend and the wiring without Ollama/whisper/piper present.

use async_trait::async_trait;

use super::{Conversant, Dictionary, ProviderResult, Synthesizer, Transcriber};
use crate::domain::{Definition, Sense, Turn};

/// Pretends to hear a fixed phrase.
pub struct StubTranscriber;

#[async_trait]
impl Transcriber for StubTranscriber {
    async fn transcribe(&self, _audio: &[u8], _mime: &str) -> ProviderResult<String> {
        Ok("Bonjour, comment ça va ?".to_string())
    }
}

/// Echoes a canned French reply regardless of input.
pub struct StubConversant;

#[async_trait]
impl Conversant for StubConversant {
    async fn reply(&self, _system: &str, _history: &[Turn]) -> ProviderResult<String> {
        Ok("Je suis un assistant de démonstration. Installe Ollama pour me faire parler vraiment !".to_string())
    }
}

/// Returns a minimal valid (silent) WAV so the frontend's audio path works.
pub struct StubSynthesizer;

#[async_trait]
impl Synthesizer for StubSynthesizer {
    async fn synthesize(&self, _text: &str) -> ProviderResult<Vec<u8>> {
        Ok(silent_wav())
    }
}

/// Returns a canned definition so the save-word flow works without network.
pub struct StubDictionary;

#[async_trait]
impl Dictionary for StubDictionary {
    async fn define(&self, word: &str) -> ProviderResult<Definition> {
        Ok(Definition {
            word: word.to_string(),
            source: "Démonstration".to_string(),
            source_url: String::new(),
            senses: vec![Sense {
                part_of_speech: "Nom".to_string(),
                gloss: format!("Définition de démonstration pour « {word} »."),
            }],
        })
    }
}

/// A 44-byte WAV header describing zero samples — valid, silent, tiny.
fn silent_wav() -> Vec<u8> {
    let sample_rate: u32 = 16_000;
    let mut wav = Vec::with_capacity(44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&36u32.to_le_bytes()); // chunk size = 36 + data(0)
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // channels = mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&0u32.to_le_bytes()); // data size = 0
    wav
}
