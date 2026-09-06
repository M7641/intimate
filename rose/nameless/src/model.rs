use std::path::PathBuf;

use anyhow::Result;
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_llama::ModelWeights;
use tokenizers::Tokenizer;

use crate::ui;

const GGUF_URL: &str = "https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf";
const TOKENIZER_URL: &str =
    "https://huggingface.co/TinyLlama/TinyLlama-1.1B-Chat-v1.0/resolve/main/tokenizer.json";

pub struct CandleModel {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
}

pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Download a file from URL to cache_dir, skipping if already present.
fn download_cached(url: &str, filename: &str) -> Result<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let cache_dir = PathBuf::from(home).join(".cache").join("nameless");
    std::fs::create_dir_all(&cache_dir)?;

    let path = cache_dir.join(filename);
    if path.exists() {
        ui::status(&format!("cached: {}", path.display()));
        return Ok(path);
    }

    ui::status(&format!("downloading {filename}..."));
    let client = reqwest::blocking::Client::builder().timeout(None).build()?;
    let mut response = client.get(url).send()?;
    if !response.status().is_success() {
        anyhow::bail!("download failed: HTTP {}", response.status());
    }

    let mut file = std::fs::File::create(&path)?;
    let bytes_copied = std::io::copy(&mut response, &mut file)?;
    ui::status(&format!(
        "saved: {} ({:.1} MB)",
        path.display(),
        bytes_copied as f64 / 1_048_576.0
    ));

    Ok(path)
}

impl CandleModel {
    pub fn load() -> Result<Self> {
        ui::status("model weights:");
        let gguf_path = download_cached(GGUF_URL, "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf")?;

        ui::status("tokenizer:");
        let tok_path = download_cached(TOKENIZER_URL, "tokenizer.json")?;

        let device = Self::select_device()?;
        ui::status(&format!("device: {:?}", device));

        ui::status("loading model into memory...");
        let mut file = std::fs::File::open(&gguf_path)?;
        let content = gguf_file::Content::read(&mut file)?;
        let model = ModelWeights::from_gguf(content, &mut file, &device)?;

        let tokenizer = Tokenizer::from_file(tok_path)
            .map_err(|e| anyhow::anyhow!("tokenizer load failed: {e}"))?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    fn select_device() -> Result<Device> {
        #[cfg(feature = "metal")]
        {
            Ok(Device::new_metal(0)?)
        }
        #[cfg(not(feature = "metal"))]
        {
            Ok(Device::Cpu)
        }
    }

    pub fn chat(
        &mut self,
        messages: &[ChatMessage],
        max_tokens: usize,
        temperature: Option<f64>,
    ) -> Result<String> {
        let prompt = Self::apply_chat_template(messages);
        self.generate(&prompt, max_tokens, temperature)
    }

    fn generate(
        &mut self,
        prompt: &str,
        max_tokens: usize,
        temperature: Option<f64>,
    ) -> Result<String> {
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("tokenize failed: {e}"))?;
        let prompt_tokens = encoding.get_ids().to_vec();
        let prompt_len = prompt_tokens.len();

        let mut logits_processor = LogitsProcessor::new(299792458, temperature, None);

        // Forward pass on the full prompt
        let input = Tensor::new(prompt_tokens.as_slice(), &self.device)?.unsqueeze(0)?;
        let logits = self.model.forward(&input, 0)?;
        let logits = logits.squeeze(0)?.squeeze(0)?;
        let mut next_token = logits_processor.sample(&logits)?;

        let mut output_tokens: Vec<u32> = vec![next_token];
        let eos_token = self.tokenizer.token_to_id("</s>").unwrap_or(2);

        // Autoregressive generation
        for i in 0..max_tokens.saturating_sub(1) {
            if next_token == eos_token {
                break;
            }
            let input = Tensor::new(&[next_token], &self.device)?.unsqueeze(0)?;
            let logits = self.model.forward(&input, prompt_len + i + 1)?;
            let logits = logits.squeeze(0)?.squeeze(0)?;
            next_token = logits_processor.sample(&logits)?;
            output_tokens.push(next_token);
        }

        // Remove trailing EOS
        if output_tokens.last() == Some(&eos_token) {
            output_tokens.pop();
        }

        let text = self
            .tokenizer
            .decode(&output_tokens, true)
            .map_err(|e| anyhow::anyhow!("decode failed: {e}"))?;

        Ok(text)
    }

    /// TinyLlama uses the Zephyr chat template:
    /// <|system|>\n{content}</s>\n<|user|>\n{content}</s>\n<|assistant|>\n
    fn apply_chat_template(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    prompt.push_str("<|system|>\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("</s>\n");
                }
                "user" => {
                    prompt.push_str("<|user|>\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("</s>\n");
                }
                "assistant" => {
                    prompt.push_str("<|assistant|>\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("</s>\n");
                }
                _ => {}
            }
        }
        prompt.push_str("<|assistant|>\n");
        prompt
    }
}
