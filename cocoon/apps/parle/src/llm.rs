//! A small instruct model, downloaded from Hugging Face once and run locally
//! with candle. Adapted from `augur`'s loader; this one streams text back via
//! a callback so the UI can show the story appearing.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2;
use tokenizers::Tokenizer;

use crate::gguf_tokenizer;

/// Qwen2.5 0.5B Instruct, 8-bit. About 675 MB, multilingual, and small enough
/// to write a paragraph in a few seconds on a CPU.
const MODEL_URL: &str = "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q8_0.gguf";
const MODEL_FILE: &str = "qwen2.5-0.5b-instruct-q8_0.gguf";

pub struct Model {
    weights: quantized_qwen2::ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    eos: Vec<u32>,
}

impl Model {
    /// Download (if needed) and load the model, reporting progress through
    /// `status` so the UI has something to show during a 675 MB fetch.
    pub fn load(status: &mut dyn FnMut(String)) -> Result<Self> {
        let path = download_cached(MODEL_URL, MODEL_FILE, status)?;
        status("loading model…".into());

        let device = select_device()?;
        tracing::info!("device: {device:?}");

        let mut file = std::fs::File::open(&path).context("open gguf")?;
        let content = gguf_file::Content::read(&mut file).context("read gguf header")?;
        let arch = content
            .metadata
            .get("general.architecture")
            .and_then(|v| v.to_string().ok())
            .cloned()
            .unwrap_or_default();
        if arch != "qwen2" {
            bail!("unexpected gguf architecture {arch:?}, expected qwen2");
        }

        let tokenizer = gguf_tokenizer::from_gguf_metadata(&content.metadata)?;
        let weights = quantized_qwen2::ModelWeights::from_gguf(content, &mut file, &device)
            .context("build qwen2 from gguf")?;
        let eos = ["<|im_end|>", "<|endoftext|>"]
            .iter()
            .filter_map(|t| tokenizer.token_to_id(t))
            .collect();

        Ok(Self { weights, tokenizer, device, eos })
    }

    /// Generate up to `max_tokens`, calling `on_text` with the full decoded
    /// text so far after every token. Whole-text rather than deltas because a
    /// single token can be half a multi-byte character (common in CJK); the
    /// tokenizer only produces clean text from the complete sequence.
    pub fn generate(
        &mut self,
        prompt: &str,
        max_tokens: usize,
        temperature: f64,
        on_text: &mut dyn FnMut(&str),
    ) -> Result<()> {
        let encoding = self
            .tokenizer
            .encode(prompt, false)
            .map_err(|e| anyhow::anyhow!("tokenize: {e}"))?;
        let prompt_tokens = encoding.get_ids().to_vec();
        let prompt_len = prompt_tokens.len();

        let mut sampler = LogitsProcessor::new(rand_seed(), Some(temperature), Some(0.9));

        // Prefill at offset 0 resets Qwen2's KV cache, so each story is fresh.
        let input = Tensor::new(prompt_tokens.as_slice(), &self.device)?.unsqueeze(0)?;
        let logits = self.weights.forward(&input, 0)?.squeeze(0)?;
        let mut next = sampler.sample(&logits)?;

        let mut out: Vec<u32> = Vec::with_capacity(max_tokens);
        for step in 0..max_tokens {
            if self.eos.contains(&next) {
                break;
            }
            out.push(next);

            let text = self
                .tokenizer
                .decode(&out, true)
                .map_err(|e| anyhow::anyhow!("decode: {e}"))?;
            on_text(text.trim_end_matches('\u{FFFD}'));

            let input = Tensor::new(&[next], &self.device)?.unsqueeze(0)?;
            let logits = self.weights.forward(&input, prompt_len + step)?.squeeze(0)?;
            next = sampler.sample(&logits)?;
        }
        Ok(())
    }
}

/// Qwen2.5's chat template, rendered by hand: three turns, no thinking mode.
/// The language is named in the user turn as well as the system prompt: a
/// model this small tends to answer in whatever language the request is in.
pub fn story_prompt(language: &str, request: &str) -> String {
    format!(
        "<|im_start|>system\nYou are a writer of short stories for people learning {language}. \
         Use short sentences and everyday words. \
         Reply with just the story, no title and no translation.<|im_end|>\n\
         <|im_start|>user\nWrite a short story in {language} about: {request}<|im_end|>\n\
         <|im_start|>assistant\n"
    )
}

fn rand_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42)
}

fn select_device() -> Result<Device> {
    #[cfg(feature = "cuda")]
    {
        return Ok(Device::new_cuda(0)?);
    }
    #[cfg(all(target_os = "macos", not(feature = "cuda")))]
    {
        return Ok(Device::new_metal(0)?);
    }
    #[allow(unreachable_code)]
    Ok(Device::Cpu)
}

/// Download `url` into `~/.cache/parle/<filename>` unless it is already there.
/// Streams to a `.part` file and renames at the end, so a killed download
/// never leaves a truncated GGUF that looks complete.
fn download_cached(url: &str, filename: &str, status: &mut dyn FnMut(String)) -> Result<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let cache_dir = PathBuf::from(home).join(".cache").join("parle");
    std::fs::create_dir_all(&cache_dir)?;

    let path = cache_dir.join(filename);
    if path.exists() {
        return Ok(path);
    }
    let part = cache_dir.join(format!("{filename}.part"));

    status("downloading model…".into());
    let client = reqwest::blocking::Client::builder().timeout(None).build()?;
    let mut response = client.get(url).send()?.error_for_status()?;
    let total = response.content_length();

    let mut file = std::fs::File::create(&part)?;
    let mut done: u64 = 0;
    let mut last_reported: u64 = 0;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = std::io::Read::read(&mut response, &mut buf)?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])?;
        done += n as u64;
        if done - last_reported >= 8 << 20 {
            last_reported = done;
            let mb = done as f64 / 1_048_576.0;
            status(match total {
                Some(t) => format!("downloading model… {mb:.0} / {:.0} MB", t as f64 / 1_048_576.0),
                None => format!("downloading model… {mb:.0} MB"),
            });
        }
    }
    drop(file);
    std::fs::rename(&part, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Downloads the model on first run (675 MB) and writes a short story.
    /// Run with `cargo test -p parle --release -- --ignored --nocapture`.
    #[test]
    #[ignore = "downloads a model and runs inference"]
    fn writes_a_story() {
        let mut model = Model::load(&mut |s| eprintln!("{s}")).unwrap();
        let mut out = String::new();
        let started = std::time::Instant::now();
        model
            .generate(&story_prompt("French", "a cat who is afraid of rain"), 120, 0.7, &mut |t| {
                out = t.to_string();
            })
            .unwrap();
        eprintln!("--- {:.1}s ---\n{out}", started.elapsed().as_secs_f32());
        assert!(out.split_whitespace().count() > 10, "story too short: {out:?}");
    }
}
