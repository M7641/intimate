//! Pluggable LLM judge — lifted from the `nameless` pilot's `auto_client` pattern.
//!
//! The judge is what "decides what's worth reading". It runs in the digest lane
//! (batch) and in the reflect loop (rewriting the taste profile).

use anyhow::Context;

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn prompt(&self, system: &str, user: &str) -> anyhow::Result<String>;
}

/// Uses the `claude` CLI (`claude -p`) — works with a subscription, no API key.
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

/// Work/API token set → Anthropic API, else fall back to the `claude` CLI.
pub fn auto_client() -> Box<dyn LlmClient> {
    if let Some(key) = resolve_anthropic_key() {
        tracing::info!("judge: using Anthropic API");
        Box::new(RigAnthropicClient::new(key))
    } else {
        tracing::info!("judge: using claude CLI");
        Box::new(ClaudeCliClient)
    }
}
