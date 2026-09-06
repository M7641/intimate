//! Crawl an app's own links and capture each route.
//!
//! Multi-page apps render real `<a href>` links — and those links carry valid
//! parameters (e.g. `/orders/123`), because the app generated them. So we don't
//! invent parameters: we harvest the app's links, follow same-origin ones in
//! breadth-first order up to a page/depth budget, and capture each route.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use anyhow::Result;
use chromiumoxide::Page;
use tracing::{info, warn};
use url::{Origin, Url};

use crate::capture::{dom, network::NetworkCapture, visual};
use crate::models::PageCapture;
use crate::reflect::Bundle;

/// Crawl configuration. Mirrors the `--crawl*` CLI flags.
pub struct Options {
    pub enabled: bool,
    pub max_pages: usize,
    pub max_depth: usize,
    /// Cap on captures sharing one path (e.g. `/table_info?id=…` permutations).
    pub max_per_route: usize,
}

/// Crawl from the already-loaded landing page. Returns one [`PageCapture`] per
/// visited route (excluding the entry page, which is the bundle's top level).
pub async fn run(
    page: &Page,
    entry_url: &str,
    net: &NetworkCapture,
    bundle: &Bundle,
    settle: u64,
    opts: &Options,
) -> Result<Vec<PageCapture>> {
    let origin = Url::parse(entry_url)?.origin();

    // The entry page is already captured at the top level — don't revisit it.
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(normalize(entry_url));
    // Per-path counter, so one route with many query permutations is bounded.
    let mut per_route: HashMap<String, usize> = HashMap::new();

    // Prime the delta cursors so the first page's network excludes everything
    // observed up to now (the entry page's own traffic).
    let mut seen: HashSet<String> = HashSet::new();
    let mut ws_cursor = 0usize;
    let _ = net.delta(&mut seen, &mut ws_cursor).await;

    // Seed the queue with the entry page's links.
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    enqueue_links(page, &origin, &visited, 1, &mut queue).await;

    let mut pages = Vec::new();
    while let Some((url, depth)) = queue.pop_front() {
        if pages.len() >= opts.max_pages {
            info!(
                max_pages = opts.max_pages,
                "crawl page budget reached — stopping"
            );
            break;
        }
        if !visited.insert(normalize(&url)) {
            continue; // already captured
        }
        // Bound permutations of the same route (same path, differing query).
        let key = route_key(&url);
        let count = per_route.entry(key.clone()).or_insert(0);
        if *count >= opts.max_per_route {
            info!(
                route = key,
                cap = opts.max_per_route,
                "per-route cap reached — skipping"
            );
            continue;
        }

        info!(%url, depth, captured = pages.len(), "crawling route");
        if let Err(e) = page.goto(&url).await {
            warn!(%url, error = %e, "navigation failed — skipping");
            continue;
        }
        let _ = page.wait_for_navigation().await;
        // Wait for the route's data to actually arrive before the screenshot.
        if !net
            .wait_for_idle(crate::IDLE_QUIET, Duration::from_secs(settle))
            .await
        {
            info!(%url, settle, "network still busy at timeout — capturing anyway");
        }
        *count += 1;

        let slug = format!("{:02}-{}", pages.len() + 1, slugify(&url));
        let _ = visual::full_page(page, &bundle.screenshot_path(&slug)).await;
        let (components, inputs) = dom::capture_a11y(page).await.unwrap_or_default();
        let (network, _ws) = net.delta(&mut seen, &mut ws_cursor).await;

        pages.push(PageCapture {
            url: url.clone(),
            title: page_title(page).await,
            screenshot: format!("{slug}.png"),
            components,
            inputs,
            network,
        });

        if depth < opts.max_depth {
            enqueue_links(page, &origin, &visited, depth + 1, &mut queue).await;
        }
    }

    info!(pages = pages.len(), "crawl complete");
    Ok(pages)
}

/// Collect same-origin links from the current page and push the unseen ones.
async fn enqueue_links(
    page: &Page,
    origin: &Origin,
    visited: &HashSet<String>,
    depth: usize,
    queue: &mut VecDeque<(String, usize)>,
) {
    for link in discover_links(page).await {
        if same_origin(&link, origin) && !visited.contains(&normalize(&link)) {
            queue.push_back((link, depth));
        }
    }
}

/// `a.href` resolves to an absolute URL in the browser, so the parameters the
/// app rendered come back intact.
async fn discover_links(page: &Page) -> Vec<String> {
    let js = "() => Array.from(document.querySelectorAll('a[href]')).map(a => a.href)";
    match page.evaluate_function(js).await {
        Ok(result) => result.into_value::<Vec<String>>().unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

async fn page_title(page: &Page) -> Option<String> {
    page.evaluate("document.title")
        .await
        .ok()
        .and_then(|r| r.into_value::<String>().ok())
        .filter(|s| !s.is_empty())
}

fn same_origin(url: &str, origin: &Origin) -> bool {
    Url::parse(url)
        .map(|u| &u.origin() == origin)
        .unwrap_or(false)
}

/// Drop the fragment so `/a` and `/a#x` count as one route.
fn normalize(url: &str) -> String {
    match Url::parse(url) {
        Ok(mut u) => {
            u.set_fragment(None);
            u.to_string()
        }
        Err(_) => url.to_string(),
    }
}

/// The route a URL belongs to — its path, ignoring query and fragment — so that
/// `/table_info?id=1` and `/table_info?id=2` share one bounded bucket.
fn route_key(url: &str) -> String {
    Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| url.to_string())
}

fn slugify(url: &str) -> String {
    let path = Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| url.to_string());
    let slug: String = path
        .trim_matches('/')
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if slug.is_empty() {
        "root".to_string()
    } else {
        slug
    }
}
