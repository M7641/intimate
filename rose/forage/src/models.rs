//! Serde types shared across forage.
//!
//! Where calque's models describe a *spec to rebuild from*, forage's describe
//! *what was observed and what broke*: network records carry bodies (so we can
//! judge whether a response is sensible), controls carry enough to drive them,
//! and the run meta summarises coverage.

use serde::{Deserialize, Serialize};

/// One request/response pair, correlated by CDP request id. Extends calque's
/// record with the request id (needed to fetch the body), the transport failure
/// text, and the response body itself.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NetworkRecord {
    pub request_id: String,
    pub url: String,
    pub method: Option<String>,
    pub status: Option<i64>,
    pub mime_type: Option<String>,
    /// CDP resource type (Document, Xhr, Fetch, Image, …).
    pub resource_type: Option<String>,
    /// `error_text` from `loadingFailed` — e.g. `net::ERR_CONNECTION_REFUSED`.
    pub failure: Option<String>,
    /// Truncated response body text, when retrievable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_preview: Option<String>,
    /// Parsed JSON body, when the body is JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json: Option<serde_json::Value>,
}

/// What kind of interactive control we discovered — drives how we exercise it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Select,
    Checkbox,
    Radio,
    Text,
    Number,
    Date,
    Email,
    Search,
    Button,
    Tab,
    /// Anything we won't drive (password/file inputs, unknown roles).
    Other,
}

/// The owning `<form>` of a control, when there is one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormInfo {
    pub selector: String,
    /// Lowercased HTTP method (`get`/`post`).
    pub method: String,
    /// Whether the form holds a password/file/payment field — never submitted.
    pub has_sensitive: bool,
}

/// An interactive control, discovered generically with no site knowledge, with
/// a robust selector so we can re-find it after re-navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlSpec {
    pub selector: String,
    pub kind: ControlKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<FormInfo>,
    #[serde(default)]
    pub disabled: bool,
}

/// One row of the crawl map — every URL visited and what happened there.
#[derive(Debug, Clone, Serialize)]
pub struct VisitRecord {
    pub url: String,
    pub depth: usize,
    pub status: Option<i64>,
    pub title: Option<String>,
    pub controls_found: usize,
    pub controls_exercised: usize,
    pub timed_out: bool,
}

/// Run-level summary written to the report header.
#[derive(Debug, Default, Serialize)]
pub struct RunMeta {
    pub base_url: String,
    pub started_at: String,
    pub duration_s: f64,
    pub budget_s: u64,
    pub pages_visited: usize,
    pub routes_covered: usize,
    pub controls_exercised: usize,
    pub requests_observed: usize,
    pub findings_total: usize,
    pub errors: usize,
    pub warnings: usize,
}
