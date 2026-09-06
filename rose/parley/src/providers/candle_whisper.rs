//! Transcriber running Whisper in-process via Candle — no ffmpeg, no subprocess.
//!
//! The browser sends raw PCM already at what Whisper wants (16 kHz, mono, f32),
//! so there is nothing to decode server-side: bytes → mel spectrogram → encoder
//! → greedy-decoded French text. Weights come from the HF Hub once, then run on
//! the Metal GPU, the same shape as `candle_llm.rs`.
//!
//! Scope is deliberately M1-simple: one 30-second chunk, greedy (temperature 0),
//! language forced to French, no timestamps. The temperature-fallback sweep and
//! no-speech gating in Candle's full example are quality refinements for later.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use candle_core::{Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, audio, model::Whisper, Config};
use tokenizers::Tokenizer;

use super::{ProviderResult, Transcriber};

/// 80-bin mel filterbank, as shipped with Candle's Whisper example. Whisper needs
/// these to turn a spectrogram into the log-mel input the encoder was trained on;
/// they never change, so we embed them rather than fetch them.
const MEL_FILTERS_80: &[u8] = include_bytes!("melfilters.bytes");

pub struct CandleWhisper {
    // The decoder carries a KV cache mutated during generation, so a single
    // instance is serialized behind a mutex — fine for one learner at a time.
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    model: Whisper,
    tokenizer: Tokenizer,
    config: Config,
    device: Device,
    mel_filters: Vec<f32>,
    /// Logit mask (0 or -inf per vocab id) that suppresses tokens Whisper should
    /// never emit here — added to the logits before picking the next token.
    suppress: Tensor,
    /// The fixed prompt tokens: start-of-transcript, language, task, no-timestamps.
    prompt: Vec<u32>,
    eot: u32,
    /// Token budget per utterance (`max_target_positions / 2`, as in Whisper).
    max_new: usize,
}

impl CandleWhisper {
    /// Download (cached) and load a multilingual Whisper model. Blocking and slow
    /// on first run — call once at startup. `repo` is an HF model id such as
    /// `openai/whisper-small` (80-mel, multilingual).
    pub fn load(repo: &str) -> ProviderResult<Self> {
        let api = hf_hub::api::sync::Api::new()?;
        let model_repo = api.model(repo.to_string());

        tracing::info!(%repo, "downloading Whisper weights (first run only)");
        let config_path = model_repo.get("config.json")?;
        let tokenizer_path = model_repo.get("tokenizer.json")?;
        let weights_path = model_repo.get("model.safetensors")?;

        let config: Config = serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;
        if config.num_mel_bins != 80 {
            anyhow::bail!(
                "parley bundles the 80-bin mel filterbank; {repo} uses {} mel bins \
                 (use an 80-mel model like openai/whisper-small)",
                config.num_mel_bins
            );
        }

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("tokenizer load: {e}"))?;

        let device = Device::new_metal(0)?;
        let mel_filters = read_f32_le(MEL_FILTERS_80);

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], m::DTYPE, &device)?
        };
        let model = Whisper::load(&vb, config.clone())?;

        // French, transcribe (not translate), without timestamps.
        let prompt = vec![
            token(&tokenizer, m::SOT_TOKEN)?,
            token(&tokenizer, "<|fr|>")?,
            token(&tokenizer, m::TRANSCRIBE_TOKEN)?,
            token(&tokenizer, m::NO_TIMESTAMPS_TOKEN)?,
        ];
        let eot = token(&tokenizer, m::EOT_TOKEN)?;
        let suppress = suppress_mask(&config, &device)?;
        let max_new = config.max_target_positions / 2;

        tracing::info!("candle Whisper ready (metal)");
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                model,
                tokenizer,
                config,
                device,
                mel_filters,
                suppress,
                prompt,
                eot,
                max_new,
            })),
        })
    }
}

impl Inner {
    /// Greedy-decode mono 16 kHz f32 PCM into text (first 30 s if longer).
    fn transcribe_pcm(&mut self, samples: Vec<f32>) -> ProviderResult<String> {
        // `pcm_to_mel` already pads to a whole number of chunks (frames are a
        // multiple of 1500, plus one trailing pad chunk), so we feed the raw
        // samples — padding them ourselves would overshoot the 3000-frame window.
        let mel = audio::pcm_to_mel(&self.config, &samples, &self.mel_filters);
        let n_mels = self.config.num_mel_bins;
        let frames = mel.len() / n_mels;
        let mel = Tensor::from_vec(mel, (1, n_mels, frames), &self.device)?;

        // The encoder's positional embedding is exactly N_FRAMES/2 long, so cap
        // the mel at 30 s (N_FRAMES). Real speech sits in the leading frames; the
        // rest is `pcm_to_mel`'s trailing pad, so narrowing loses nothing here.
        let mel = if frames > m::N_FRAMES {
            mel.narrow(2, 0, m::N_FRAMES)?
        } else {
            mel
        };

        // Fresh cache per utterance — no carry-over between turns.
        self.model.reset_kv_cache();
        let audio_features = self.model.encoder.forward(&mel, true)?;

        let mut tokens = self.prompt.clone();
        for step in 0..self.max_new {
            let tokens_t = Tensor::new(tokens.as_slice(), &self.device)?.unsqueeze(0)?;
            // Only the first pass fills the KV cache from the whole prompt.
            let ys = self.model.decoder.forward(&tokens_t, &audio_features, step == 0)?;

            let (_, seq_len, _) = ys.dims3()?;
            let logits = self
                .model
                .decoder
                .final_linear(&ys.i((..1, seq_len - 1..))?)?
                .i(0)?
                .i(0)?;
            let logits = logits.broadcast_add(&self.suppress)?;

            let next = argmax(&logits)?;
            if next == self.eot {
                break;
            }
            tokens.push(next);
        }

        // `skip_special_tokens = true` drops the prompt/EOT tokens, leaving text.
        let text = self
            .tokenizer
            .decode(&tokens, true)
            .map_err(|e| anyhow::anyhow!("whisper decode: {e}"))?;
        Ok(text.trim().to_string())
    }
}

#[async_trait]
impl Transcriber for CandleWhisper {
    async fn transcribe(&self, audio: &[u8], _mime: &str) -> ProviderResult<String> {
        // `audio` is little-endian f32 samples (mono 16 kHz) captured by the
        // browser — see `frontend/src/recorder.ts`.
        let samples = read_f32_le(audio);
        let inner = self.inner.clone();
        // Inference is CPU/GPU-bound and holds a mutex; keep it off the async
        // reactor by running it on the blocking pool.
        tokio::task::spawn_blocking(move || {
            let mut guard = inner.lock().unwrap();
            guard.transcribe_pcm(samples)
        })
        .await?
    }
}

/// Read a little-endian `f32` slice from raw bytes (trailing partial ignored).
fn read_f32_le(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Look up a token id, erroring clearly if the tokenizer lacks it.
fn token(tokenizer: &Tokenizer, tok: &str) -> ProviderResult<u32> {
    tokenizer
        .token_to_id(tok)
        .ok_or_else(|| anyhow::anyhow!("whisper tokenizer missing {tok}"))
}

/// Build the per-vocab logit mask: `-inf` for tokens Whisper's own config marks
/// as never-emit, `0` everywhere else. Added to logits at each decode step.
fn suppress_mask(config: &Config, device: &Device) -> ProviderResult<Tensor> {
    let mask: Vec<f32> = (0..config.vocab_size as u32)
        .map(|i| {
            if config.suppress_tokens.contains(&i) {
                f32::NEG_INFINITY
            } else {
                0.0
            }
        })
        .collect();
    Ok(Tensor::new(mask.as_slice(), device)?)
}

/// Argmax over a 1-D logits tensor → token id.
fn argmax(logits: &Tensor) -> ProviderResult<u32> {
    let v: Vec<f32> = logits.to_vec1()?;
    let id = v
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(i, _)| i as u32)
        .ok_or_else(|| anyhow::anyhow!("empty logits"))?;
    Ok(id)
}
