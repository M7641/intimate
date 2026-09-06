//! Behavioural sampling — execute a plan against the live API and record
//! request/response pairs. These pairs become the Axum characterization tests:
//! status codes, response shapes and validation behaviour OpenAPI omits.
//!
//! Sampling is sequential and rate-limited — it sends real traffic to a live
//! system, so it stays polite by construction.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{anyhow, Result};
use reqwest::{Client, Method};
use serde_json::Value;
use tracing::{info, warn};

use crate::api::plan::{SamplePlan, Variant};
use crate::models::Sample;

const RESPONSE_PREVIEW_LEN: usize = 2000;

/// Run every endpoint/variant in the plan, returning one [`Sample`] each.
pub async fn run(client: &Client, base_url: &str, plan: &SamplePlan) -> Result<Vec<Sample>> {
    let base = base_url.trim_end_matches('/');
    let auth = resolve_auth_headers(plan)?;
    let delay = Duration::from_secs_f64(1.0 / plan.rate_limit_rps.max(0.1));
    info!(
        plan = plan.name,
        endpoints = plan.endpoints.len(),
        rps = plan.rate_limit_rps,
        "sampling"
    );

    let mut samples = Vec::new();
    for endpoint in &plan.endpoints {
        let method = Method::from_bytes(endpoint.method.to_uppercase().as_bytes())
            .map_err(|_| anyhow!("invalid method: {}", endpoint.method))?;

        for (i, variant) in endpoint.variants.iter().enumerate() {
            tokio::time::sleep(delay).await; // politeness

            let path = substitute(&endpoint.path, variant);
            let url = format!("{base}{path}");
            let name = sample_name(&endpoint.method, &endpoint.path, i);

            match sample_one(client, &method, &url, variant, &auth, &name).await {
                Ok(sample) => {
                    info!(name = sample.name, status = sample.status, "sampled");
                    samples.push(sample);
                }
                Err(e) => warn!(%url, error = %e, "sample failed"),
            }
        }
    }
    Ok(samples)
}

async fn sample_one(
    client: &Client,
    method: &Method,
    url: &str,
    variant: &Variant,
    auth: &BTreeMap<String, String>,
    name: &str,
) -> Result<Sample> {
    let mut req = client.request(method.clone(), url);

    let query: Vec<(String, String)> = variant
        .query
        .iter()
        .map(|(k, v)| (k.clone(), value_to_string(v)))
        .collect();
    if !query.is_empty() {
        req = req.query(&query);
    }
    if let Some(body) = &variant.body {
        req = req.json(body);
    }
    for (name, value) in auth {
        req = req.header(name.as_str(), value.as_str());
    }

    let resp = req.send().await?;
    let status = resp.status().as_u16();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let text = resp.text().await?;
    let (response_json, response_preview) = match serde_json::from_str::<Value>(&text) {
        Ok(json) => (Some(json), None),
        Err(_) => (None, Some(preview(&text))),
    };

    Ok(Sample {
        name: name.to_string(),
        method: method.to_string(),
        url: url.to_string(),
        request_body: variant.body.clone(),
        status,
        content_type,
        response_json,
        response_preview,
    })
}

/// Resolve the plan's auth into concrete request headers, reading every secret
/// from the environment.
fn resolve_auth_headers(plan: &SamplePlan) -> Result<BTreeMap<String, String>> {
    let mut headers = BTreeMap::new();
    let Some(auth) = &plan.auth else {
        return Ok(headers);
    };

    if let Some(var) = &auth.bearer_env {
        let token = std::env::var(var).map_err(|_| anyhow!("bearer env var {var} not set"))?;
        headers.insert("Authorization".to_string(), format!("Bearer {token}"));
    }
    for (header, var) in &auth.header_env {
        let value = std::env::var(var).map_err(|_| anyhow!("header env var {var} not set"))?;
        headers.insert(header.clone(), value);
    }
    Ok(headers)
}

fn substitute(path: &str, variant: &Variant) -> String {
    let mut out = path.to_string();
    for (key, value) in &variant.path_params {
        out = out.replace(&format!("{{{key}}}"), &value_to_string(value));
    }
    out
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn sample_name(method: &str, path: &str, index: usize) -> String {
    let slug: String = path
        .trim_matches('/')
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let slug = if slug.is_empty() {
        "root".to_string()
    } else {
        slug
    };
    format!("{}-{slug}-{index}", method.to_lowercase())
}

fn preview(text: &str) -> String {
    if text.len() <= RESPONSE_PREVIEW_LEN {
        text.to_string()
    } else {
        format!("{}…", &text[..RESPONSE_PREVIEW_LEN])
    }
}
