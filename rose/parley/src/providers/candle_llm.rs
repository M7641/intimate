//! Conversant running a quantized LLM in-process via Candle (Hugging Face's Rust
//! ML framework). No external server: the GGUF weights are pulled from the Hub
//! once, loaded into memory, and inference runs on the Metal GPU.
//!
//! Default model: Qwen2.5-3B-Instruct (GGUF Q4). It's multilingual, ungated, and
//! light enough to iterate on. Swap to a larger model by changing the env vars.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2::ModelWeights as Qwen2;
use tokenizers::Tokenizer;

use super::{Conversant, ProviderResult};
use crate::domain::{Role, Turn};

/// How the conversation is framed for the model. Qwen uses ChatML, whose turn
/// boundary token `<|im_end|>` is also our stop signal.
const IM_END: &str = "<|im_end|>";

pub struct CandleLlm {
    // Generation mutates the model (KV cache) and is not concurrency-safe, so a
    // single instance is serialized behind a mutex. Fine for one learner at a time.
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    model: Qwen2,
    tokenizer: Tokenizer,
    device: Device,
    eos: u32,
    max_tokens: usize,
}

impl CandleLlm {
    /// Download (cached) and load the model. Blocking and slow on first run — call
    /// once at startup.
    ///
    /// `gguf_repo`/`gguf_file` locate the quantized weights; `tokenizer_repo` holds
    /// the matching `tokenizer.json`.
    pub fn load(gguf_repo: &str, gguf_file: &str, tokenizer_repo: &str) -> ProviderResult<Self> {
        let api = hf_hub::api::sync::Api::new()?;

        tracing::info!(repo = gguf_repo, file = gguf_file, "downloading GGUF weights (first run only)");
        let weights_path = api.model(gguf_repo.to_string()).get(gguf_file)?;
        let tokenizer_path = api.model(tokenizer_repo.to_string()).get("tokenizer.json")?;

        let device = Device::new_metal(0)?;

        let mut file = std::fs::File::open(&weights_path)?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| e.with_path(&weights_path))?;
        let model = Qwen2::from_gguf(content, &mut file, &device)?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("tokenizer load: {e}"))?;
        let eos = tokenizer
            .token_to_id(IM_END)
            .ok_or_else(|| anyhow::anyhow!("tokenizer missing {IM_END}"))?;

        tracing::info!("candle LLM ready (metal)");
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner { model, tokenizer, device, eos, max_tokens: 512 })),
        })
    }
}

/// Render system prompt + history into a Qwen ChatML prompt ending with an open
/// assistant turn for the model to complete.
fn build_chatml(system: &str, history: &[Turn]) -> String {
    let mut s = format!("<|im_start|>system\n{system}<|im_end|>\n");
    for turn in history {
        let role = match turn.role {
            Role::Learner => "user",
            Role::Tutor => "assistant",
        };
        s.push_str(&format!("<|im_start|>{role}\n{}<|im_end|>\n", turn.text));
    }
    s.push_str("<|im_start|>assistant\n");
    s
}

impl Inner {
    /// Greedy-ish sampling generation: prefill the whole prompt from position 0,
    /// then decode token by token until `<|im_end|>` or the token budget.
    fn generate(&mut self, prompt: &str) -> ProviderResult<String> {
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("encode: {e}"))?;
        let tokens = encoding.get_ids();

        let mut logits_processor = LogitsProcessor::new(42, Some(0.7), Some(0.9));
        let mut generated: Vec<u32> = Vec::new();

        // Prefill: process the full prompt, sample the first reply token.
        let input = Tensor::new(tokens, &self.device)?.unsqueeze(0)?;
        let logits = self.model.forward(&input, 0)?.squeeze(0)?;
        let mut next = logits_processor.sample(&logits)?;
        generated.push(next);

        // Decode loop.
        for index in 0..self.max_tokens {
            if next == self.eos {
                break;
            }
            let input = Tensor::new(&[next], &self.device)?.unsqueeze(0)?;
            let logits = self.model.forward(&input, tokens.len() + index)?.squeeze(0)?;
            // A small repeat penalty keeps it from looping on a phrase.
            let start = generated.len().saturating_sub(64);
            let logits =
                candle_transformers::utils::apply_repeat_penalty(&logits, 1.1, &generated[start..])?;
            next = logits_processor.sample(&logits)?;
            generated.push(next);
        }

        let text = self
            .tokenizer
            .decode(&generated, true)
            .map_err(|e| anyhow::anyhow!("decode: {e}"))?;
        Ok(text.trim().to_string())
    }
}

#[async_trait]
impl Conversant for CandleLlm {
    async fn reply(&self, system: &str, history: &[Turn]) -> ProviderResult<String> {
        let prompt = build_chatml(system, history);
        // Inference is GPU/CPU-bound and blocking; block_in_place keeps it off the
        // async reactor without requiring the model to be `Send` across threads.
        tokio::task::block_in_place(|| {
            let mut inner = self.inner.lock().unwrap();
            inner.generate(&prompt)
        })
    }
}
