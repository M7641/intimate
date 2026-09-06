//! Turn observations into findings — the "is this sensible?" judgements.
//!
//! Two entry points: `page` classifies a visit (its navigation, traffic, and
//! console), `interaction` classifies the result of exercising a control. Both
//! share `classify_request`, which judges a single network record.

use crate::explore::controls::InteractionOutcome;
use crate::explore::page::PageVisit;
use crate::findings::{Category, Finding, Severity};
use crate::models::NetworkRecord;

pub fn page(visit: &PageVisit) -> Vec<Finding> {
    let mut out = Vec::new();
    let url = &visit.url;

    if let Some(err) = &visit.nav_error {
        out.push(Finding::new(
            Category::DeadLink,
            Severity::Error,
            url,
            format!("navigation failed: {err}"),
        ));
        return out; // nothing else to judge — the page never loaded
    }
    if visit.timed_out {
        out.push(Finding::new(
            Category::PageTimeout,
            Severity::Warning,
            url,
            "network never went idle before the timeout",
        ));
    }

    for rec in &visit.network {
        if let Some(f) = classify_request(rec, url) {
            out.push(f);
        }
    }
    out.extend(console_findings(&visit.console, url));

    if visit.has_error_state {
        out.push(Finding::new(
            Category::PageError,
            Severity::Warning,
            url,
            "page rendered an error/alert element on load",
        ));
    }
    out
}

pub fn interaction(outcome: &InteractionOutcome, url: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let what = describe(outcome);

    if let Some(err) = &outcome.action_error {
        // The action itself failed — lower severity: the control may simply be
        // conditionally enabled or have moved.
        out.push(
            Finding::new(
                Category::ControlError,
                Severity::Warning,
                url,
                format!("{what} failed: {err}"),
            )
            .with_control(outcome.control.clone()),
        );
    }
    if let Some(reason) = &outcome.broke_page {
        // A collapsed page (white-screen) is a hard break; a mere error/alert
        // element appearing after the action is softer — many apps show "no
        // results" alerts on a legitimate empty filter.
        let severity = if reason.contains("white-screen") {
            Severity::Error
        } else {
            Severity::Warning
        };
        out.push(
            Finding::new(
                Category::ControlError,
                severity,
                url,
                format!("{what} broke the page: {reason}"),
            )
            .with_control(outcome.control.clone()),
        );
    }

    for rec in &outcome.network {
        if let Some(f) = classify_request(rec, url) {
            out.push(f.with_control(outcome.control.clone()));
        }
    }
    out.extend(
        console_findings(&outcome.console, url)
            .into_iter()
            .map(|f| f.with_control(outcome.control.clone())),
    );
    out
}

fn describe(o: &InteractionOutcome) -> String {
    let what = o
        .control
        .label
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&o.control.selector);
    format!("{} ({})", o.action, truncate(what, 40))
}

/// Judge a single network record. `None` means "looks fine".
fn classify_request(rec: &NetworkRecord, page_url: &str) -> Option<Finding> {
    let rtype = rec.resource_type.as_deref().unwrap_or("");
    let is_document = rtype == "Document";
    let is_xhr = matches!(rtype, "Xhr" | "Fetch");

    // Transport failure (DNS, refused, reset, …). Aborts are usually benign
    // (navigations cancel in-flight requests), so don't flag those.
    if let Some(err) = &rec.failure {
        if err.contains("ABORTED") {
            return None;
        }
        let (cat, sev) = if is_document {
            (Category::DeadLink, Severity::Error)
        } else if is_xhr {
            (Category::FailedXhr, Severity::Error)
        } else {
            (Category::BrokenResource, Severity::Warning)
        };
        return Some(
            Finding::new(cat, sev, page_url, format!("request failed: {err}"))
                .with_request(rec.url.clone()),
        );
    }

    if let Some(status) = rec.status {
        if status >= 500 {
            return Some(
                Finding::new(
                    Category::ServerError,
                    Severity::Error,
                    page_url,
                    format!("{} returned {status}", short_url(&rec.url)),
                )
                .with_request(rec.url.clone())
                .with_status(Some(status)),
            );
        }
        if status >= 400 {
            let (cat, sev) = if is_document {
                (Category::DeadLink, Severity::Error)
            } else if is_xhr {
                (Category::FailedXhr, Severity::Error)
            } else {
                (Category::BrokenResource, Severity::Warning)
            };
            return Some(
                Finding::new(
                    cat,
                    sev,
                    page_url,
                    format!("{} returned {status}", short_url(&rec.url)),
                )
                .with_request(rec.url.clone())
                .with_status(Some(status)),
            );
        }
    }

    // 2xx XHR/fetch — judge the body.
    if is_xhr && rec.status.map(|s| (200..300).contains(&s)).unwrap_or(false) {
        let says_json = rec
            .mime_type
            .as_deref()
            .map(|m| m.contains("json"))
            .unwrap_or(false);

        if says_json && rec.json.is_none() && rec.body_preview.is_some() {
            return Some(
                Finding::new(
                    Category::MalformedJson,
                    Severity::Warning,
                    page_url,
                    format!("{} is declared JSON but did not parse", short_url(&rec.url)),
                )
                .with_request(rec.url.clone()),
            );
        }
        if let Some(json) = &rec.json {
            if is_error_shaped(json) {
                return Some(
                    Finding::new(
                        Category::SoftError,
                        Severity::Warning,
                        page_url,
                        format!("{} returned 200 with an error body", short_url(&rec.url)),
                    )
                    .with_request(rec.url.clone()),
                );
            }
            if is_empty_json(json) && looks_like_data(&rec.url) {
                return Some(
                    Finding::new(
                        Category::EmptyResponse,
                        Severity::Warning,
                        page_url,
                        format!("{} returned empty data", short_url(&rec.url)),
                    )
                    .with_request(rec.url.clone()),
                );
            }
        }
    }
    None
}

fn console_findings(entries: &[crate::capture::console::ConsoleEntry], url: &str) -> Vec<Finding> {
    entries
        .iter()
        .filter(|e| e.is_error())
        .map(|e| {
            let (cat, sev) = if e.level == "exception" {
                (Category::JsException, Severity::Error)
            } else {
                (Category::ConsoleError, Severity::Warning)
            };
            Finding::new(cat, sev, url, truncate(&e.text, 200)).with_console(e.text.clone())
        })
        .collect()
}

fn is_empty_json(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => true,
        serde_json::Value::Array(a) => a.is_empty(),
        serde_json::Value::Object(o) => o.is_empty(),
        serde_json::Value::String(s) => s.is_empty(),
        _ => false,
    }
}

fn is_error_shaped(v: &serde_json::Value) -> bool {
    let serde_json::Value::Object(o) = v else {
        return false;
    };
    for key in ["error", "errors"] {
        if let Some(val) = o.get(key) {
            if !is_empty_json(val) && !matches!(val, serde_json::Value::Bool(false)) {
                return true;
            }
        }
    }
    false
}

/// Heuristic: does this URL look like a data fetch (so emptiness is suspect)?
fn looks_like_data(url: &str) -> bool {
    let u = url.to_lowercase();
    [
        "api", "/data", "list", "search", "query", "results", "items",
    ]
    .iter()
    .any(|k| u.contains(k))
}

fn short_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(u) => {
            let q = u.query().map(|q| format!("?{q}")).unwrap_or_default();
            truncate(&format!("{}{q}", u.path()), 80)
        }
        Err(_) => truncate(url, 80),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}
