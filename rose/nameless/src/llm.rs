use std::sync::{Arc, Mutex};

use anyhow::Context;

use crate::model::{CandleModel, ChatMessage};

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn prompt(&self, system: &str, user: &str) -> anyhow::Result<String>;
}

/// Uses the `claude` CLI (`claude -p`) — works with a subscription, no API key needed.
pub struct ClaudeCliClient;

#[async_trait::async_trait]
impl LlmClient for ClaudeCliClient {
    async fn prompt(&self, system: &str, user: &str) -> anyhow::Result<String> {
        let full_prompt = format!("{system}\n\n{user}");
        let output = tokio::process::Command::new("claude")
            .args(["-p", &full_prompt, "--output-format", "text"])
            .output()
            .await
            .context("failed to run `claude` CLI — is it installed?")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("claude CLI failed: {stderr}");
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

/// Resolve the Anthropic API key, preferring the shared work token used across
/// our tools (`ANTHROPIC_API_TOKEN_WORK`) and falling back to the SDK-standard
/// `ANTHROPIC_API_KEY`. Returns `None` when neither is set (so we drop to the CLI).
pub fn resolve_anthropic_key() -> Option<String> {
    ["ANTHROPIC_API_TOKEN_WORK", "ANTHROPIC_API_KEY"]
        .into_iter()
        .find_map(|var| std::env::var(var).ok().filter(|v| !v.is_empty()))
}

/// Uses the Anthropic API directly via rig, with a key resolved by
/// [`resolve_anthropic_key`] (`ANTHROPIC_API_TOKEN_WORK` or `ANTHROPIC_API_KEY`).
pub struct RigAnthropicClient {
    api_key: String,
}

impl RigAnthropicClient {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[async_trait::async_trait]
impl LlmClient for RigAnthropicClient {
    async fn prompt(&self, system: &str, user: &str) -> anyhow::Result<String> {
        use rig::client::{CompletionClient, ProviderClient};
        use rig::completion::Prompt;
        use rig::providers::anthropic;

        // Build with the explicit key rather than `from_env()`, so the work token
        // is honoured without depending on `ANTHROPIC_API_KEY` being set.
        let client = anthropic::Client::from_val(self.api_key.clone());
        let agent = client
            .agent(anthropic::completion::CLAUDE_3_5_SONNET)
            .preamble(system)
            .build();

        let response: String = agent
            .prompt(user.to_string())
            .await
            .context("anthropic API call failed")?;

        Ok(response.trim().to_string())
    }
}

/// Uses a local TinyLlama model via Candle for fully offline inference.
pub struct LocalCandleClient {
    model: Arc<Mutex<CandleModel>>,
}

impl LocalCandleClient {
    pub fn new(model: CandleModel) -> Self {
        Self {
            model: Arc::new(Mutex::new(model)),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for LocalCandleClient {
    async fn prompt(&self, system: &str, user: &str) -> anyhow::Result<String> {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user.to_string(),
            },
        ];

        let model = Arc::clone(&self.model);
        let result = tokio::task::spawn_blocking(move || {
            let mut model = model.lock().unwrap();
            model.chat(&messages, 512, Some(0.7))
        })
        .await??;

        Ok(result.trim().to_string())
    }
}

/// Pick a backend: `--local` → Candle, work/API token → Anthropic API, else → claude CLI.
pub fn auto_client(local: bool) -> anyhow::Result<Box<dyn LlmClient>> {
    if local {
        crate::ui::status("loading local model (first run downloads ~637 MB)...");
        let model = CandleModel::load()?;
        crate::ui::status("local model ready");
        Ok(Box::new(LocalCandleClient::new(model)))
    } else if let Some(key) = resolve_anthropic_key() {
        crate::ui::status("using Anthropic API");
        Ok(Box::new(RigAnthropicClient::new(key)))
    } else {
        crate::ui::status("using claude CLI");
        Ok(Box::new(ClaudeCliClient))
    }
}
