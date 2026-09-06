//! Discover, fetch and parse the OpenAPI spec — the structural golden.
//!
//! If the caller names a spec (`--openapi`), we honour it (URL or local file).
//! Otherwise we probe well-known paths. When nothing is found the API path still
//! runs: sampling alone yields behaviour, and a spec can be synthesized later.

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, info};

use crate::models::Endpoint;

/// Conventional locations a running service exposes its spec at.
const WELL_KNOWN: &[&str] = &[
    "/openapi.json",
    "/swagger.json",
    "/v3/api-docs",
    "/api-docs",
    "/swagger/v1/swagger.json",
];

/// A located spec: where it came from, and its parsed JSON.
pub struct Spec {
    pub source: String,
    pub doc: Value,
}

/// Resolve a caller-named spec — an `http(s)` URL is fetched, anything else is
/// read as a local file path.
pub async fn fetch_named(client: &Client, spec: &str) -> Result<Spec> {
    if spec.starts_with("http://") || spec.starts_with("https://") {
        let doc = get_json(client, spec).await?;
        Ok(Spec {
            source: spec.to_string(),
            doc,
        })
    } else {
        let text = std::fs::read_to_string(spec).with_context(|| format!("reading {spec}"))?;
        let doc = serde_json::from_str(&text).with_context(|| format!("parsing {spec}"))?;
        Ok(Spec {
            source: spec.to_string(),
            doc,
        })
    }
}

/// Probe well-known paths under `base_url`; return the first spec found.
pub async fn discover(client: &Client, base_url: &str) -> Option<Spec> {
    let base = base_url.trim_end_matches('/');
    for path in WELL_KNOWN {
        let url = format!("{base}{path}");
        debug!(%url, "probing for OpenAPI spec");
        match get_json(client, &url).await {
            Ok(doc) if looks_like_openapi(&doc) => {
                info!(%url, "found OpenAPI spec");
                return Some(Spec { source: url, doc });
            }
            Ok(_) => debug!(%url, "responded but not an OpenAPI document"),
            Err(e) => debug!(%url, error = %e, "no spec here"),
        }
    }
    None
}

/// Flatten the spec's `paths` object into a list of endpoints.
pub fn parse_endpoints(doc: &Value) -> Vec<Endpoint> {
    let mut endpoints = Vec::new();
    let Some(paths) = doc.get("paths").and_then(Value::as_object) else {
        return endpoints;
    };
    for (path, item) in paths {
        let Some(methods) = item.as_object() else {
            continue;
        };
        for (method, op) in methods {
            // Skip non-operation keys like "parameters" or "$ref".
            if !is_http_method(method) {
                continue;
            }
            let summary = op
                .get("summary")
                .or_else(|| op.get("description"))
                .and_then(Value::as_str)
                .map(str::to_string);
            endpoints.push(Endpoint {
                path: path.clone(),
                method: method.to_uppercase(),
                summary,
            });
        }
    }
    endpoints
}

async fn get_json(client: &Client, url: &str) -> Result<Value> {
    let resp = client.get(url).send().await?.error_for_status()?;
    Ok(resp.json().await?)
}

fn looks_like_openapi(doc: &Value) -> bool {
    doc.get("openapi").is_some() || doc.get("swagger").is_some()
}

fn is_http_method(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "get" | "put" | "post" | "delete" | "patch" | "head" | "options" | "trace"
    )
}
