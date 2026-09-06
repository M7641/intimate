//! The OpenAI-compatible `/v1/completions` surface Zed's edit prediction calls.
//!
//! Only the fields Zed actually sends are modelled. Inference is offloaded to a
//! blocking thread so the async runtime stays responsive while the GPU works.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/completions", post(completions))
        .route("/health", get(|| async { "ok" }))
        .with_state(state)
}

#[derive(Deserialize)]
pub struct CompletionRequest {
    prompt: String,
    #[serde(default = "default_max_tokens")]
    max_tokens: usize,
    #[serde(default)]
    temperature: f64,
    #[serde(default)]
    stop: Option<Stop>,
}

fn default_max_tokens() -> usize {
    256
}

/// `stop` arrives as either a single string or an array of them.
#[derive(Deserialize)]
#[serde(untagged)]
enum Stop {
    One(String),
    Many(Vec<String>),
}

impl Stop {
    fn into_vec(self) -> Vec<String> {
        match self {
            Stop::One(s) => vec![s],
            Stop::Many(v) => v,
        }
    }
}

#[derive(Serialize)]
pub struct CompletionResponse {
    id: String,
    object: &'static str,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Serialize)]
struct Choice {
    text: String,
    index: usize,
    finish_reason: &'static str,
}

#[derive(Serialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

async fn completions(
    State(state): State<AppState>,
    Json(req): Json<CompletionRequest>,
) -> Result<Json<CompletionResponse>, ApiError> {
    let stops = req.stop.map(Stop::into_vec).unwrap_or_default();
    let model = state.model.clone();
    let prompt_log = state.prompt_log.clone();
    let CompletionRequest {
        prompt,
        max_tokens,
        temperature,
        ..
    } = req;

    // Generation is blocking, GPU-bound work: run it off the async runtime.
    let completion = tokio::task::spawn_blocking(move || {
        let result = {
            let mut guard = model.lock().expect("model mutex poisoned");
            guard.generate(&prompt, max_tokens, temperature, &stops)
        };
        // Capture the raw prompt + completion for format alignment.
        if let (Some(path), Ok(c)) = (&prompt_log, &result) {
            append_prompt_log(path, &prompt, c);
        }
        result
    })
    .await
    .map_err(|e| ApiError(format!("inference task panicked: {e}")))?
    .map_err(|e| ApiError(format!("generation failed: {e}")))?;

    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(Json(CompletionResponse {
        id: format!("cmpl-augur-{created}"),
        object: "text_completion",
        created,
        model: state.model_name,
        usage: Usage {
            prompt_tokens: completion.prompt_tokens,
            completion_tokens: completion.completion_tokens,
            total_tokens: completion.prompt_tokens + completion.completion_tokens,
        },
        choices: vec![Choice {
            text: completion.text,
            index: 0,
            finish_reason: completion.finish_reason,
        }],
    }))
}

/// Append one request as a JSON line: the raw prompt Zed sent and what the
/// model produced. Best-effort — a logging failure must not fail the request.
fn append_prompt_log(path: &std::path::Path, prompt: &str, c: &crate::model::Completion) {
    use std::io::Write;
    let line = serde_json::json!({
        "prompt": prompt,
        "completion": c.text,
        "finish_reason": c.finish_reason,
        "prompt_tokens": c.prompt_tokens,
        "completion_tokens": c.completion_tokens,
    });
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| writeln!(f, "{line}"));
}

/// Minimal error -> 500 JSON, matching the OpenAI error envelope shape.
struct ApiError(String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!("{}", self.0);
        let body = Json(serde_json::json!({
            "error": { "message": self.0, "type": "augur_error" }
        }));
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}
