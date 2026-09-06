//! The finding model — the report's payload.
//!
//! Findings dedupe by fingerprint so the same broken asset across many pages
//! collapses to one entry that records every URL it was `seen_on`.

use std::collections::HashMap;

use serde::Serialize;

use crate::frontier::route_key;
use crate::models::ControlSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    DeadLink,
    BrokenResource,
    ServerError,
    FailedXhr,
    MalformedJson,
    EmptyResponse,
    SoftError,
    ConsoleError,
    JsException,
    ControlError,
    PageError,
    PageTimeout,
    /// A control the safety policy declined to exercise (info only).
    Skipped,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::DeadLink => "Dead link",
            Category::BrokenResource => "Broken resource",
            Category::ServerError => "Server error (5xx)",
            Category::FailedXhr => "Failed XHR/fetch",
            Category::MalformedJson => "Malformed JSON",
            Category::EmptyResponse => "Empty response",
            Category::SoftError => "Error-shaped 200",
            Category::ConsoleError => "Console error",
            Category::JsException => "Uncaught exception",
            Category::ControlError => "Control error",
            Category::PageError => "Page error state",
            Category::PageTimeout => "Page timeout",
            Category::Skipped => "Skipped (safety)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub category: Category,
    pub severity: Severity,
    /// The page where this was observed.
    pub url: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub console_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    /// Other page URLs where the same finding recurred.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub seen_on: Vec<String>,
}

impl Finding {
    pub fn new(
        category: Category,
        severity: Severity,
        url: &str,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            category,
            severity,
            url: url.to_string(),
            detail: detail.into(),
            http_status: None,
            request_url: None,
            console_text: None,
            control: None,
            screenshot: None,
            seen_on: Vec::new(),
        }
    }

    pub fn with_status(mut self, status: Option<i64>) -> Self {
        self.http_status = status;
        self
    }

    pub fn with_request(mut self, url: impl Into<String>) -> Self {
        self.request_url = Some(url.into());
        self
    }

    pub fn with_console(mut self, text: impl Into<String>) -> Self {
        self.console_text = Some(text.into());
        self
    }

    pub fn with_control(mut self, control: ControlSpec) -> Self {
        self.control = Some(control);
        self
    }

    /// Collapses repeats: same category, route, request, control, and message
    /// shape are treated as one finding.
    fn fingerprint(&self) -> String {
        let req = self.request_url.as_deref().unwrap_or("");
        let ctrl = self
            .control
            .as_ref()
            .map(|c| c.selector.as_str())
            .unwrap_or("");
        let detail: String = self.detail.chars().take(60).collect();
        format!(
            "{:?}|{}|{}|{}|{}",
            self.category,
            route_key(&self.url),
            req,
            ctrl,
            detail
        )
    }
}

/// A deduping collection of findings.
#[derive(Default)]
pub struct Findings {
    items: Vec<Finding>,
    index: HashMap<String, usize>,
}

impl Findings {
    pub fn add(&mut self, finding: Finding) {
        let fp = finding.fingerprint();
        if let Some(&i) = self.index.get(&fp) {
            let existing = &mut self.items[i];
            if existing.url != finding.url && !existing.seen_on.contains(&finding.url) {
                existing.seen_on.push(finding.url);
            }
            return;
        }
        self.index.insert(fp, self.items.len());
        self.items.push(finding);
    }

    pub fn extend(&mut self, findings: impl IntoIterator<Item = Finding>) {
        for f in findings {
            self.add(f);
        }
    }

    /// Attach a screenshot to the most recently added finding of a category, if
    /// it doesn't already have one.
    pub fn attach_screenshot(&mut self, category: Category, file: &str) {
        if let Some(f) = self
            .items
            .iter_mut()
            .rev()
            .find(|f| f.category == category && f.screenshot.is_none())
        {
            f.screenshot = Some(file.to_string());
        }
    }

    pub fn items(&self) -> &[Finding] {
        &self.items
    }

    pub fn count(&self, severity: Severity) -> usize {
        self.items.iter().filter(|f| f.severity == severity).count()
    }

    pub fn has_visual_error(&self, since: usize) -> bool {
        self.items[since..]
            .iter()
            .any(|f| matches!(f.category, Category::PageError | Category::ControlError))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}
