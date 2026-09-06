//! Sample-plan format — which endpoints to hit, with which input variants, and
//! how politely. A plan can be authored (YAML, see README) or auto-derived from
//! the OpenAPI endpoints when none is supplied.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::models::Endpoint;

fn default_rps() -> f64 {
    2.0
}

#[derive(Debug, Deserialize)]
pub struct SamplePlan {
    pub name: String,
    /// Politeness — requests per second against the live system.
    #[serde(default = "default_rps")]
    pub rate_limit_rps: f64,
    #[serde(default)]
    pub auth: Option<Auth>,
    pub endpoints: Vec<PlanEndpoint>,
}

/// How to authenticate the sampling requests. Both forms read secrets from the
/// environment, never the plan file — so plans stay safe to commit.
#[derive(Debug, Default, Deserialize)]
pub struct Auth {
    /// Env var holding a bearer token → `Authorization: Bearer <value>`.
    #[serde(default)]
    pub bearer_env: Option<String>,
    /// Extra headers as `header-name: env-var-name` — the env var holds the
    /// value (e.g. `X-Api-Key: API_KEY` reads `$API_KEY`).
    #[serde(default)]
    pub header_env: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct PlanEndpoint {
    pub path: String,
    #[serde(default = "get_method")]
    pub method: String,
    #[serde(default)]
    pub variants: Vec<Variant>,
}

fn get_method() -> String {
    "GET".to_string()
}

/// One concrete invocation of an endpoint — values to surface a behaviour.
#[derive(Debug, Default, Deserialize)]
pub struct Variant {
    #[serde(default)]
    pub path_params: BTreeMap<String, Value>,
    #[serde(default)]
    pub query: BTreeMap<String, Value>,
    #[serde(default)]
    pub body: Option<Value>,
}

pub fn load(path: &Path) -> Result<SamplePlan> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_yaml_ng::from_str(&text)?)
}

/// Build a minimal plan from discovered endpoints: one no-input variant per GET
/// endpoint with no path parameters. Parameterized endpoints need an authored
/// plan (we can't invent ids), so they're left out of the auto plan.
pub fn auto_plan(endpoints: &[Endpoint]) -> SamplePlan {
    let endpoints = endpoints
        .iter()
        .filter(|e| e.method == "GET" && !e.path.contains('{'))
        .map(|e| PlanEndpoint {
            path: e.path.clone(),
            method: "GET".to_string(),
            variants: vec![Variant::default()],
        })
        .collect();
    SamplePlan {
        name: "auto".to_string(),
        rate_limit_rps: default_rps(),
        auth: None,
        endpoints,
    }
}
