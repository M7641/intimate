//! A quantized model loaded once, generating raw text completions on Metal.
//!
//! The GGUF's `general.architecture` picks the backend: Qwen2 (e.g. Sweep
//! Next-Edit, Qwen2.5-Coder) or Qwen3. Both expose the same `forward`, so the
//! generation loop is shared. Adapted for a long-lived server: every request
//! starts with a prefill at offset 0 so one prediction never inherits
//! another's KV state.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::{quantized_qwen2, quantized_qwen3};
use tokenizers::Tokenizer;

use crate::gguf_tokenizer;

/// The model architectures we can load, behind one uniform interface.
enum Backend {
    Qwen2(quantized_qwen2::ModelWeights),
    Qwen3(quantized_qwen3::ModelWeights),
}

impl Backend {
    fn forward(&mut self, input: &Tensor, offset: usize) -> Result<Tensor> {
        Ok(match self {
            Backend::Qwen2(m) => m.forward(input, offset)?,
            Backend::Qwen3(m) => m.forward(input, offset)?,
        })
    }

    /// Reset KV state between requests. Qwen2's hand-rolled cache resets itself
    /// on the offset-0 prefill, so it needs nothing here; Qwen3's appending
    /// cache must be cleared explicitly.
    fn clear_kv_cache(&mut self) {
        match self {
            Backend::Qwen2(_) => {}
            Backend::Qwen3(m) => m.clear_kv_cache(),
        }
    }
}

/// Where to fetch the weights and tokenizer from. Filled by the CLI.
pub struct ModelConfig {
    pub model_url: String,
    pub model_file: String,
    /// Optional external tokenizer. When `None`, the tokenizer is rebuilt from
    /// the GGUF's own vocabulary — the right default, since it always matches.
    pub tokenizer_url: Option<String>,
}

pub struct CandleModel {
    model: Backend,
    tokenizer: Tokenizer,
    device: Device,
    eos_tokens: Vec<u32>,
}

/// The outcome of one generation: the produced text plus why it stopped and
/// token counts for the response's `usage` block.
pub struct Completion {
    pub text: String,
    pub finish_reason: &'static str,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
}

impl CandleModel {
    pub fn load(cfg: &ModelConfig) -> Result<Self> {
        let gguf_path = download_cached(&cfg.model_url, &cfg.model_file)?;

        let device = select_device()?;
        tracing::info!("device: {device:?}");

        let mut file = std::fs::File::open(&gguf_path).context("open gguf")?;
        let content = gguf_file::Content::read(&mut file).context("read gguf header")?;

        // The header names the architecture; pick the matching backend later.
        let arch = content
            .metadata
            .get("general.architecture")
            .and_then(|v| v.to_string().ok())
            .cloned()
            .unwrap_or_default();
        tracing::info!("gguf architecture: {arch}");

        // Build the tokenizer before `from_gguf` consumes `content`. Prefer the
        // GGUF's own vocabulary (always matches the weights); fall back to an
        // explicit URL only if one was given.
        let tokenizer = match &cfg.tokenizer_url {
            Some(url) => {
                let tok_file = format!("{}.tokenizer.json", cfg.model_file);
                let tok_path = download_cached(url, &tok_file)?;
                tracing::info!("tokenizer: external {tok_file}");
                Tokenizer::from_file(&tok_path)
                    .map_err(|e| anyhow::anyhow!("tokenizer load: {e}"))?
            }
            None => {
                tracing::info!("tokenizer: rebuilt from gguf vocabulary");
                gguf_tokenizer::from_gguf_metadata(&content.metadata)?
            }
        };

        let model = match arch.as_str() {
            "qwen2" => Backend::Qwen2(
                quantized_qwen2::ModelWeights::from_gguf(content, &mut file, &device)
                    .context("build qwen2 from gguf")?,
            ),
            "qwen3" => Backend::Qwen3(
                quantized_qwen3::ModelWeights::from_gguf(content, &mut file, &device)
                    .context("build qwen3 from gguf")?,
            ),
            other => bail!("unsupported gguf architecture: {other:?} (expected qwen2 or qwen3)"),
        };

        // Tokens that end a completion. `<|file_sep|>` bounds Sweep Next-Edit's
        // format, `<|fim_pad|>` ends a FIM fill, the rest end generic Qwen
        // completions. Only those present in the vocab are kept.
        let eos_tokens = ["<|endoftext|>", "<|im_end|>", "<|file_sep|>", "<|fim_pad|>"]
            .iter()
            .filter_map(|t| tokenizer.token_to_id(t))
            .collect::<Vec<_>>();
        tracing::info!("model ready (eos tokens: {eos_tokens:?})");

        Ok(Self {
            model,
            tokenizer,
            device,
            eos_tokens,
        })
    }

    /// Generate a completion for `prompt`, stopping at `max_tokens`, at any EOS
    /// token, or as soon as the decoded text contains one of `stops`.
    pub fn generate(
        &mut self,
        prompt: &str,
        max_tokens: usize,
        temperature: f64,
        stops: &[String],
    ) -> Result<Completion> {
        // Fresh request: drop any KV state from the previous one.
        self.model.clear_kv_cache();

        // Raw completion / FIM: do not inject special tokens — the prompt is
        // already exactly what the client formatted.
        let encoding = self
            .tokenizer
            .encode(prompt, false)
            .map_err(|e| anyhow::anyhow!("tokenize: {e}"))?;
        let prompt_tokens = encoding.get_ids().to_vec();
        if prompt_tokens.is_empty() {
            bail!("empty prompt");
        }
        let prompt_len = prompt_tokens.len();

        // temperature <= 0 means greedy (argmax); a positive value samples.
        let temp = (temperature > 0.0).then_some(temperature);
        let mut logits = LogitsProcessor::new(299_792_458, temp, None);

        // Prefill: one forward pass over the whole prompt.
        let input = Tensor::new(prompt_tokens.as_slice(), &self.device)?.unsqueeze(0)?;
        let last = self.model.forward(&input, 0)?.squeeze(0)?;
        let mut next = logits.sample(&last)?;

        let mut out: Vec<u32> = Vec::with_capacity(max_tokens);
        let mut finish_reason = "length";

        for step in 0..max_tokens {
            if self.eos_tokens.contains(&next) {
                finish_reason = "stop";
                break;
            }
            out.push(next);

            // Decode-so-far to honour string stop sequences (what Zed sends).
            if !stops.is_empty() {
                let text = self.decode(&out)?;
                if let Some(cut) = stops.iter().filter_map(|s| text.find(s.as_str())).min() {
                    return Ok(self.finished(
                        text[..cut].to_string(),
                        "stop",
                        prompt_len,
                        out.len(),
                    ));
                }
            }

            // Position of this token: the prompt filled 0..prompt_len-1, so the
            // first generated token sits at prompt_len, the next at prompt_len+1…
            let input = Tensor::new(&[next], &self.device)?.unsqueeze(0)?;
            let step_logits = self.model.forward(&input, prompt_len + step)?.squeeze(0)?;
            next = logits.sample(&step_logits)?;
        }

        let text = self.decode(&out)?;
        Ok(self.finished(text, finish_reason, prompt_len, out.len()))
    }

    fn finished(
        &self,
        text: String,
        finish_reason: &'static str,
        prompt_tokens: usize,
        completion_tokens: usize,
    ) -> Completion {
        Completion {
            text,
            finish_reason,
            prompt_tokens,
            completion_tokens,
        }
    }

    fn decode(&self, tokens: &[u32]) -> Result<String> {
        self.tokenizer
            .decode(tokens, true)
            .map_err(|e| anyhow::anyhow!("decode: {e}"))
    }
}

fn select_device() -> Result<Device> {
    // Metal is compiled in on macOS (see Cargo.toml's target-gated deps), so it
    // is selected automatically here — no feature flag to remember.
    #[cfg(target_os = "macos")]
    {
        Ok(Device::new_metal(0)?)
    }
    #[cfg(not(target_os = "macos"))]
    {
        tracing::warn!("non-macOS build — running on CPU, expect very high latency");
        Ok(Device::Cpu)
    }
}

/// Download `url` into `~/.cache/augur/<filename>`, reusing it if already there.
fn download_cached(url: &str, filename: &str) -> Result<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let cache_dir = PathBuf::from(home).join(".cache").join("augur");
    std::fs::create_dir_all(&cache_dir)?;

    let path = cache_dir.join(filename);
    if path.exists() {
        return Ok(path);
    }

    tracing::info!("downloading {filename}...");
    let client = reqwest::blocking::Client::builder().timeout(None).build()?;
    let mut response = client.get(url).send()?;
    if !response.status().is_success() {
        bail!("download {filename} failed: HTTP {}", response.status());
    }

    let mut file = std::fs::File::create(&path)?;
    let bytes = std::io::copy(&mut response, &mut file)?;
    tracing::info!("saved {filename} ({:.1} MB)", bytes as f64 / 1_048_576.0);
    Ok(path)
}
