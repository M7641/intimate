//! UI interaction recipes — declarative steps replayed against the live app to
//! capture reactivity: which input change produces which network delta. That
//! delta is the reactive contract a React rebuild must satisfy.

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Result};
use chromiumoxide::Page;
use serde::Deserialize;
use tracing::info;

use crate::capture::{network::NetworkCapture, visual};
use crate::models::Interaction;
use crate::reflect::Bundle;

/// Pause after a triggering action before reading its network delta.
const DELTA_SETTLE: Duration = Duration::from_millis(1500);

#[derive(Debug)]
pub struct Recipe {
    pub name: String,
    pub steps: Vec<Step>,
}

/// A replayable step. Runtime form — see [`StepWire`] for the on-disk shape.
#[derive(Debug)]
pub enum Step {
    Goto(String),
    WaitFor(String),
    Screenshot(String),
    Select { selector: String, value: String },
    Click(String),
    CaptureDelta(String),
}

/// On-disk recipe: each step is a single-key map, e.g. `{ goto: "..." }` or
/// `{ select: { selector, value } }` — matching the README's YAML.
#[derive(Deserialize)]
struct RecipeWire {
    name: String,
    steps: Vec<StepWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StepWire {
    #[serde(default)]
    goto: Option<String>,
    #[serde(default)]
    wait_for: Option<String>,
    #[serde(default)]
    screenshot: Option<String>,
    #[serde(default)]
    select: Option<SelectArgs>,
    #[serde(default)]
    click: Option<String>,
    #[serde(default)]
    capture_delta: Option<String>,
}

#[derive(Deserialize)]
struct SelectArgs {
    selector: String,
    value: String,
}

impl StepWire {
    fn into_step(self) -> Result<Step> {
        if let Some(url) = self.goto {
            Ok(Step::Goto(url))
        } else if let Some(state) = self.wait_for {
            Ok(Step::WaitFor(state))
        } else if let Some(name) = self.screenshot {
            Ok(Step::Screenshot(name))
        } else if let Some(args) = self.select {
            Ok(Step::Select {
                selector: args.selector,
                value: args.value,
            })
        } else if let Some(selector) = self.click {
            Ok(Step::Click(selector))
        } else if let Some(name) = self.capture_delta {
            Ok(Step::CaptureDelta(name))
        } else {
            bail!("recipe step has no recognized action");
        }
    }
}

pub fn load(path: &Path) -> Result<Recipe> {
    let text = std::fs::read_to_string(path)?;
    let wire: RecipeWire = serde_yaml_ng::from_str(&text)?;
    let steps = wire
        .steps
        .into_iter()
        .map(StepWire::into_step)
        .collect::<Result<Vec<_>>>()?;
    Ok(Recipe {
        name: wire.name,
        steps,
    })
}

/// Replay a recipe against an already-loaded page, recording one [`Interaction`]
/// per `capture_delta` step. The network capture must already be running.
pub async fn run(
    page: &Page,
    recipe: &Recipe,
    net: &NetworkCapture,
    bundle: &Bundle,
    settle: u64,
) -> Result<Vec<Interaction>> {
    info!(
        recipe = recipe.name,
        steps = recipe.steps.len(),
        "replaying recipe"
    );

    // Prime the cursors so the first delta excludes initial-load traffic.
    let mut seen: HashSet<String> = HashSet::new();
    let mut ws_cursor = 0usize;
    let _ = net.delta(&mut seen, &mut ws_cursor).await;

    let mut interactions = Vec::new();
    for step in &recipe.steps {
        match step {
            Step::Goto(url) => {
                page.goto(url).await?;
                page.wait_for_navigation().await?;
            }
            Step::WaitFor(_state) => {
                tokio::time::sleep(Duration::from_secs(settle)).await;
            }
            Step::Screenshot(name) => {
                visual::full_page(page, &bundle.screenshot_path(name)).await?;
            }
            Step::Select { selector, value } => set_value(page, selector, value).await?,
            Step::Click(selector) => {
                page.find_element(selector).await?.click().await?;
            }
            Step::CaptureDelta(name) => {
                tokio::time::sleep(DELTA_SETTLE).await;
                let (network_delta, websocket_delta) = net.delta(&mut seen, &mut ws_cursor).await;
                info!(
                    step = name,
                    requests = network_delta.len(),
                    "captured delta"
                );
                interactions.push(Interaction {
                    step: name.clone(),
                    network_delta,
                    websocket_delta,
                });
            }
        }
    }
    Ok(interactions)
}

/// Set a form control's value and fire input/change so the app's reactive layer
/// (Shiny, React-controlled inputs, …) sees it as a real user edit.
async fn set_value(page: &Page, selector: &str, value: &str) -> Result<()> {
    let js = format!(
        "() => {{ const el = document.querySelector({sel}); \
         if (el) {{ el.value = {val}; \
         el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
         el.dispatchEvent(new Event('change', {{ bubbles: true }})); }} }}",
        sel = serde_json::to_string(selector)?,
        val = serde_json::to_string(value)?,
    );
    page.evaluate_function(js).await?;
    Ok(())
}
