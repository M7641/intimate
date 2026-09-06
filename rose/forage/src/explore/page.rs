//! Visit one URL: navigate, let it settle, then harvest links, controls, and
//! the traffic + console output the load produced.

use std::collections::HashSet;

use chromiumoxide::Page;
use tracing::debug;

use crate::budget::Budget;
use crate::capture::console::{ConsoleCapture, ConsoleEntry};
use crate::capture::dom;
use crate::capture::network::NetworkCapture;
use crate::explore::controls::IDLE_QUIET;
use crate::frontier::normalize;
use crate::models::{ControlSpec, NetworkRecord};

pub struct PageVisit {
    pub url: String,
    pub title: Option<String>,
    /// Status of the document request for this URL, if observed.
    pub status: Option<i64>,
    /// `goto` itself failed (DNS, connection refused, …).
    pub nav_error: Option<String>,
    pub timed_out: bool,
    pub links: Vec<String>,
    pub controls: Vec<ControlSpec>,
    pub network: Vec<NetworkRecord>,
    pub console: Vec<ConsoleEntry>,
    pub has_error_state: bool,
}

#[allow(clippy::too_many_arguments)]
pub async fn visit(
    page: &Page,
    url: &str,
    depth: usize,
    net: &NetworkCapture,
    console: &ConsoleCapture,
    budget: &Budget,
    net_seen: &mut HashSet<String>,
    console_cursor: &mut usize,
) -> PageVisit {
    // Prime deltas so this page reports only its own traffic.
    let _ = net.delta(net_seen).await;
    let _ = console.delta(console_cursor).await;

    let nav_error = match tokio::time::timeout(budget.action_timeout(), page.goto(url)).await {
        Ok(Ok(_)) => {
            let _ = page.wait_for_navigation().await;
            None
        }
        Ok(Err(e)) => Some(e.to_string()),
        Err(_) => Some("navigation timed out".to_string()),
    };

    let timed_out =
        nav_error.is_none() && !net.wait_for_idle(IDLE_QUIET, budget.action_timeout()).await;
    net.hydrate_bodies().await;

    let network = net.delta(net_seen).await;
    let console_entries = console.delta(console_cursor).await;

    let (links, controls, state) = if nav_error.is_none() {
        (
            dom::discover_links(page).await,
            dom::discover_controls(page).await,
            dom::page_state(page).await,
        )
    } else {
        Default::default()
    };

    // The document request carries the page's own HTTP status.
    let status = network
        .iter()
        .find(|r| {
            r.resource_type.as_deref() == Some("Document") && normalize(&r.url) == normalize(url)
        })
        .or_else(|| {
            network
                .iter()
                .find(|r| r.resource_type.as_deref() == Some("Document"))
        })
        .and_then(|r| r.status);

    debug!(
        %url,
        depth,
        links = links.len(),
        controls = controls.len(),
        requests = network.len(),
        "visited"
    );

    PageVisit {
        url: url.to_string(),
        title: if nav_error.is_none() {
            dom::current_title(page).await
        } else {
            None
        },
        status,
        nav_error,
        timed_out,
        links,
        controls,
        network,
        console: console_entries,
        has_error_state: state.has_error,
    }
}
