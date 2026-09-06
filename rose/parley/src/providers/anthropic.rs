//! Conversant backed by Anthropic's Claude, over the Messages API.
//!
//! Unlike the local providers, this one talks to a cloud endpoint and needs an
//! API key. It is a raw-HTTP client: Anthropic ships no official Rust SDK, so we
//! POST to /v1/messages with `reqwest` the same way `ollama.rs` does. The one
//! shape difference: Claude takes the pedagogy prompt in a top-level `system`
//! field rather than as a message, which maps cleanly onto `reply(system, ...)`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Conversant, ProviderResult};
use crate::domain::{Role, Turn};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

/// Default model. Opus 4.8 is the most capable model; a tutor recast is short,
/// so we leave thinking off (omitted) for fast, terse replies.
const DEFAULT_MODEL: &str = "claude-opus-4-8";

/// Replies stay short by design (principle 1: comprehensible input, one notch
/// above the learner), so a small output cap is plenty and bounds latency.
const MAX_TOKENS: u32 = 512;

pub struct Anthropic {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl Anthropic {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// The default model, overridable via `PARLEY_ANTHROPIC_MODEL`.
    pub fn default_model() -> String {
        std::env::var("PARLEY_ANTHROPIC_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
    }
}

/// One message in Claude's chat format. The system prompt is NOT a message here
/// — it rides on the top-level `system` field of the request.
#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

/// Claude returns content as a list of blocks; a plain text reply is one block
/// of type "text". We pull the text out of the first text block.
#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

/// Map our domain role onto Claude's chat roles.
fn role_str(role: Role) -> &'static str {
    match role {
        Role::Learner => "user",
        Role::Tutor => "assistant",
    }
}

#[async_trait]
impl Conversant for Anthropic {
    async fn reply(&self, system: &str, history: &[Turn]) -> ProviderResult<String> {
        let messages = history
            .iter()
            .map(|turn| ChatMessage { role: role_str(turn.role), content: &turn.text })
            .collect();

        let body = MessagesRequest {
            model: &self.model,
            max_tokens: MAX_TOKENS,
            system,
            messages,
        };

        let resp = self
            .client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<MessagesResponse>()
            .await?;

        let reply = resp
            .content
            .into_iter()
            .find(|block| block.kind == "text")
            .map(|block| block.text.trim().to_string())
            .unwrap_or_default();

        Ok(reply)
    }
}
