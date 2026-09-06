//! Serde types for the reflection bundle — shared across the UI and API paths.
//!
//! `reflection.json` is the primary artefact a skill consumes. It is kept
//! deliberately stable so other regeneration targets can plug in later (see the
//! "bundle as lingua franca" note in the README).

use serde::{Deserialize, Serialize};

/// The structured spec emitted by a single capture run.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Reflection {
    pub meta: Meta,
    /// Component inventory from the accessibility tree (roles + labels).
    pub components: Vec<Component>,
    /// Interactive controls — the inputs a recipe can drive.
    pub inputs: Vec<InputControl>,
    /// XHR/fetch/document requests observed during the capture.
    pub network: Vec<NetworkRecord>,
    /// WebSocket frames observed (the reactive channel for e.g. R Shiny).
    pub websocket: Vec<WsFrame>,
    /// Per-recipe-step interaction deltas — the reactive contract.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interactions: Vec<Interaction>,
    /// Additional routes captured by crawling. The entry page is the top-level
    /// fields above; each crawled route is one entry here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<PageCapture>,
    // Future, per README: data_endpoints, design_tokens.
}

/// One route reached by crawling — a per-page spec for a React route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageCapture {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Screenshot filename within `screenshots/`.
    pub screenshot: String,
    pub components: Vec<Component>,
    pub inputs: Vec<InputControl>,
    /// Requests this route triggered (delta over pages visited before it).
    pub network: Vec<NetworkRecord>,
}

/// One node of the accessibility tree worth rebuilding as a React component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// An interactive control discovered in the a11y tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputControl {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// The observed effect of one recipe step: which step ran, and what network
/// traffic it triggered. This is the reactive contract a rebuild must satisfy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interaction {
    pub step: String,
    /// Requests newly observed during this step (delta over prior steps).
    pub network_delta: Vec<NetworkRecord>,
    /// WebSocket frames newly observed during this step.
    pub websocket_delta: Vec<WsFrame>,
}

// ── API path ────────────────────────────────────────────────────────────────

/// The structured spec for the API path. OpenAPI gives the structure; samples
/// give the behaviour OpenAPI omits (real shapes, status codes, validation).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ApiReflection {
    pub meta: ApiMeta,
    /// Endpoints declared by the OpenAPI spec (if one was found).
    pub endpoints: Vec<Endpoint>,
    /// Observed request/response pairs from sampling.
    pub samples: Vec<Sample>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ApiMeta {
    pub base_url: String,
    pub captured_at: String,
    /// Where the OpenAPI spec was found (or fetched from), if any.
    pub openapi_source: Option<String>,
    pub endpoint_count: usize,
    pub sample_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub path: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// One observed request/response pair — a characterization-test seed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<serde_json::Value>,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Parsed JSON response, when the body is JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_json: Option<serde_json::Value>,
    /// Truncated text response, when the body is not JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_preview: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Meta {
    pub url: String,
    /// RFC3339 timestamp of the capture.
    pub captured_at: String,
    /// Heuristic framework guess (e.g. "shiny", "superset"), if any.
    pub framework: Option<String>,
    pub viewport: Viewport,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 900,
        }
    }
}

/// One request/response pair, correlated by CDP request id.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NetworkRecord {
    pub url: String,
    pub method: Option<String>,
    pub status: Option<i64>,
    pub mime_type: Option<String>,
    /// CDP resource type (Document, Xhr, Fetch, Image, …).
    pub resource_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WsDirection {
    Sent,
    Received,
}

/// One WebSocket frame. The Shiny protocol rides here: inputs `Sent`, outputs
/// `Received`. Payload is previewed (truncated) to keep the bundle readable;
/// full payloads live in the HAR once that lands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsFrame {
    /// CDP request id of the owning WebSocket connection.
    pub request_id: String,
    pub direction: WsDirection,
    pub opcode: f64,
    pub payload_preview: String,
}
