//! Conversant backed by a local Ollama server (default model: Mistral).
//!
//! Ollama exposes an OpenAI-ish chat API at http://localhost:11434. We send the
//! system prompt plus the conversation and read back one reply. Non-streaming for
//! M1 — streaming tokens is an M2 nicety once the loop works.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Conversant, ProviderResult};
use crate::domain::{Role, Turn};

pub struct Ollama {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl Ollama {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }
}

/// One message in Ollama's chat format.
#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

/// Map our domain role onto Ollama's chat roles.
fn role_str(role: Role) -> &'static str {
    match role {
        Role::Learner => "user",
        Role::Tutor => "assistant",
    }
}

#[async_trait]
impl Conversant for Ollama {
    async fn reply(&self, system: &str, history: &[Turn]) -> ProviderResult<String> {
        let mut messages = Vec::with_capacity(history.len() + 1);
        messages.push(ChatMessage { role: "system", content: system });
        for turn in history {
            messages.push(ChatMessage { role: role_str(turn.role), content: &turn.text });
        }

        let body = ChatRequest { model: &self.model, messages, stream: false };

        let resp = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<ChatResponse>()
            .await?;

        Ok(resp.message.content.trim().to_string())
    }
}
