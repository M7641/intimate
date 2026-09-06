//! Synthesizer backed by Piper TTS.
//!
//! Piper reads text on stdin and writes a WAV file. We hand it the tutor's reply
//! and read the audio back. French voices live as `<voice>.onnx` next to a
//! `<voice>.onnx.json` config that Piper finds automatically.

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::{ProviderResult, Synthesizer};

pub struct Piper {
    /// Path to a `.onnx` voice model (e.g. fr_FR-siwis-medium.onnx).
    voice_path: PathBuf,
}

impl Piper {
    pub fn new(voice_path: impl Into<PathBuf>) -> Self {
        Self { voice_path: voice_path.into() }
    }
}

#[async_trait]
impl Synthesizer for Piper {
    async fn synthesize(&self, text: &str) -> ProviderResult<Vec<u8>> {
        let mut out = std::env::temp_dir();
        out.push(format!("parley-tts-{}.wav", uuid::Uuid::new_v4()));

        let mut child = Command::new("piper")
            .arg("-m")
            .arg(&self.voice_path)
            .arg("-f")
            .arg(&out)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        // Feed the text, then close stdin so Piper starts synthesising.
        child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("piper stdin unavailable"))?
            .write_all(text.as_bytes())
            .await?;

        let status = child.wait().await?;
        if !status.success() {
            anyhow::bail!("piper exited with status {status}");
        }

        let bytes = tokio::fs::read(&out).await?;
        let _ = tokio::fs::remove_file(&out).await;
        Ok(bytes)
    }
}
