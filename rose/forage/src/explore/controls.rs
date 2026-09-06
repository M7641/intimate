//! Exercise a single control and observe whether it broke anything.
//!
//! We snapshot the page, apply one action (set a value, toggle, or click),
//! wait for the network to settle, hydrate response bodies, then read the
//! network + console deltas and compare page state. The outcome carries enough
//! for `validate` to decide what, if anything, broke.

use std::collections::HashSet;
use std::time::Duration;

use chromiumoxide::Page;
use tracing::debug;

use crate::budget::Budget;
use crate::capture::console::{ConsoleCapture, ConsoleEntry};
use crate::capture::dom::{self, PageState};
use crate::capture::network::NetworkCapture;
use crate::explore::values;
use crate::models::{ControlKind, ControlSpec, NetworkRecord};

/// Quiet window after which the network is considered idle (matches calque).
pub const IDLE_QUIET: Duration = Duration::from_millis(500);

pub struct InteractionOutcome {
    pub control: ControlSpec,
    /// What we did, for the report (e.g. "set value 'test'", "click").
    pub action: String,
    /// Requests triggered by the action.
    pub network: Vec<NetworkRecord>,
    /// Console errors/exceptions during the action.
    pub console: Vec<ConsoleEntry>,
    /// The action itself failed (selector vanished, not clickable, timed out).
    pub action_error: Option<String>,
    /// Heuristic page-breakage signal (error sentinel / white-screen).
    pub broke_page: Option<String>,
    /// Where the page ended up, if the action navigated.
    pub navigated_to: Option<String>,
    /// New same-origin links revealed by the action (e.g. pagination).
    pub revealed_links: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub async fn exercise(
    page: &Page,
    control: &ControlSpec,
    net: &NetworkCapture,
    console: &ConsoleCapture,
    budget: &Budget,
    net_seen: &mut HashSet<String>,
    console_cursor: &mut usize,
    page_token: Option<&str>,
) -> InteractionOutcome {
    // Prime the deltas so we attribute only this action's traffic.
    let _ = net.delta(net_seen).await;
    let _ = console.delta(console_cursor).await;

    let pre = dom::page_state(page).await;
    let links_before = dom::discover_links(page).await;

    let (action, action_error) = apply(page, control, page_token, budget).await;

    net.wait_for_idle(IDLE_QUIET, budget.action_timeout()).await;
    net.hydrate_bodies().await;

    let network = net.delta(net_seen).await;
    let console_entries = console.delta(console_cursor).await;
    let post = dom::page_state(page).await;
    let navigated_to = page.url().await.ok().flatten();

    let broke_page = detect_breakage(&pre, &post);

    // Links newly present after the action (e.g. a "next page" of results).
    let before: HashSet<String> = links_before.into_iter().collect();
    let revealed_links = dom::discover_links(page)
        .await
        .into_iter()
        .filter(|l| !before.contains(l))
        .collect();

    debug!(
        selector = control.selector,
        action,
        requests = network.len(),
        console = console_entries.len(),
        "exercised control"
    );

    InteractionOutcome {
        control: control.clone(),
        action,
        network,
        console: console_entries,
        action_error,
        broke_page,
        navigated_to,
        revealed_links,
    }
}

/// Perform the action, returning a description and any error.
async fn apply(
    page: &Page,
    control: &ControlSpec,
    page_token: Option<&str>,
    budget: &Budget,
) -> (String, Option<String>) {
    let timeout = budget.action_timeout();
    match control.kind {
        ControlKind::Select => match values::select_value(control) {
            Some(v) => (
                format!("select option '{v}'"),
                run(timeout, dom::set_value(page, &control.selector, &v)).await,
            ),
            None => ("select (no options)".to_string(), None),
        },
        ControlKind::Checkbox | ControlKind::Radio => (
            "toggle on".to_string(),
            run(timeout, dom::set_checked(page, &control.selector)).await,
        ),
        ControlKind::Button | ControlKind::Tab => (
            "click".to_string(),
            click(page, &control.selector, timeout).await,
        ),
        _ => match values::value_for(control, page_token) {
            Some(v) => (
                format!("set value '{v}'"),
                run(timeout, dom::set_value(page, &control.selector, &v)).await,
            ),
            None => ("no action".to_string(), None),
        },
    }
}

async fn click(page: &Page, selector: &str, timeout: Duration) -> Option<String> {
    let fut = async {
        let el = page.find_element(selector).await?;
        el.click().await?;
        Ok::<(), anyhow::Error>(())
    };
    match tokio::time::timeout(timeout, fut).await {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(e.to_string()),
        Err(_) => Some("action timed out".to_string()),
    }
}

async fn run<F>(timeout: Duration, fut: F) -> Option<String>
where
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    match tokio::time::timeout(timeout, fut).await {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(e.to_string()),
        Err(_) => Some("action timed out".to_string()),
    }
}

/// Heuristic: did the action visibly break the page?
fn detect_breakage(pre: &PageState, post: &PageState) -> Option<String> {
    if post.has_error && !pre.has_error {
        return Some("error/alert element appeared".to_string());
    }
    if pre.body_len > 200 && post.body_len < 50 {
        return Some("page content collapsed (possible white-screen)".to_string());
    }
    None
}
