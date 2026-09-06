//! API path — reflect a live API into a spec + characterization tests, targeting
//! Axum. Symmetric with the UI path: observe behaviour, freeze it as golden.
//!
//! Flow: resolve the OpenAPI spec (named or discovered) → parse endpoints →
//! run a sample plan (authored or auto-derived) → write an API reflection bundle.
//!
//! Sampling sends real traffic to a live system — only ever point it at systems
//! you own or are authorized to test. It is sequential and rate-limited.

pub mod openapi;
pub mod plan;
pub mod sample;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use chrono::Local;
use reqwest::Client;
use tracing::{info, warn};

use crate::models::{ApiMeta, ApiReflection};
use crate::reflect;

pub struct Options {
    pub openapi: Option<String>,
    pub plan: Option<PathBuf>,
    pub out: PathBuf,
}

pub async fn run_capture(base_url: &str, opts: Options) -> Result<()> {
    info!(base_url, openapi = ?opts.openapi, plan = ?opts.plan, "starting API capture");

    // Time-bounded so a slow or unreachable host fails fast with a clear error
    // instead of hanging silently.
    let client = Client::builder()
        .user_agent("calque/0.1")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()?;

    // 1. Resolve the OpenAPI spec — named wins over discovery.
    let spec = match &opts.openapi {
        Some(named) => {
            info!(named, "fetching named OpenAPI spec");
            Some(openapi::fetch_named(&client, named).await?)
        }
        None => {
            info!(base_url, "discovering OpenAPI spec at well-known paths");
            openapi::discover(&client, base_url).await
        }
    };
    match &spec {
        Some(spec) => info!(source = spec.source, "OpenAPI spec resolved"),
        None => info!("no OpenAPI spec found — will sample from the plan only"),
    }
    let endpoints = spec
        .as_ref()
        .map(|s| openapi::parse_endpoints(&s.doc))
        .unwrap_or_default();
    info!(endpoints = endpoints.len(), "parsed OpenAPI endpoints");

    // 2. Decide the sample plan — authored wins over auto-derived.
    let plan = match &opts.plan {
        Some(path) => plan::load(path)?,
        None => {
            if endpoints.is_empty() {
                warn!("no OpenAPI spec and no plan — nothing to sample");
            }
            plan::auto_plan(&endpoints)
        }
    };

    // 3. Sample the live API.
    let samples = sample::run(&client, base_url, &plan).await?;

    // 4. Write the bundle.
    info!(samples = samples.len(), "writing API reflection bundle");
    let bundle = reflect::new_api_bundle(&opts.out)?;
    if let Some(spec) = &spec {
        bundle.write_json("openapi.json", &spec.doc)?;
    }
    write_samples(&bundle, &samples)?;

    let reflection = ApiReflection {
        meta: ApiMeta {
            base_url: base_url.to_string(),
            captured_at: Local::now().to_rfc3339(),
            openapi_source: spec.as_ref().map(|s| s.source.clone()),
            endpoint_count: endpoints.len(),
            sample_count: samples.len(),
        },
        endpoints,
        samples,
    };
    bundle.write_json("reflection.json", &reflection)?;

    println!("wrote API reflection bundle to {}", bundle.dir.display());
    Ok(())
}

fn write_samples(bundle: &reflect::Bundle, samples: &[crate::models::Sample]) -> Result<()> {
    for sample in samples {
        bundle.write_sample(&format!("{}.json", sample.name), sample)?;
    }
    Ok(())
}
