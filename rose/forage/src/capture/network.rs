//! Network capture over the CDP `Network` domain.
//!
//! Adapted from calque: subscribe before navigating, correlate
//! `requestWillBeSent` (url + method) with `responseReceived` (status + mime)
//! by request id. forage adds three things calque lacks — the request id is
//! kept on each record, `loadingFailed` detail is captured, and response bodies
//! are hydrated via `Network.getResponseBody` (which only works while Chrome
//! still holds the body, i.e. after `loadingFinished` and before navigating
//! away — so `hydrate_bodies` must run right after the page settles).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use base64::Engine;
use chromiumoxide::cdp::browser_protocol::network::{
    EnableParams, EventLoadingFailed, EventLoadingFinished, EventRequestWillBeSent,
    EventResponseReceived, GetResponseBodyParams,
};
use chromiumoxide::Page;
use futures::StreamExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::models::NetworkRecord;

const BODY_PREVIEW_LEN: usize = 8192;
/// Don't hydrate bodies for non-data resources or oversized ones.
const BODY_FETCH_TIMEOUT: Duration = Duration::from_secs(3);

pub struct NetworkCapture {
    /// A clone of the page handle, used to fetch response bodies on demand.
    page: Page,
    requests: Arc<Mutex<HashMap<String, NetworkRecord>>>,
    /// HTTP requests currently in flight — drives [`wait_for_idle`].
    inflight: Arc<AtomicI64>,
    tasks: Vec<JoinHandle<()>>,
}

impl NetworkCapture {
    /// Enable the Network domain and start listening. Call before `goto`.
    pub async fn start(page: &Page) -> Result<Self> {
        page.execute(EnableParams::default()).await?;

        let requests: Arc<Mutex<HashMap<String, NetworkRecord>>> = Arc::default();
        let inflight: Arc<AtomicI64> = Arc::default();
        let mut tasks = Vec::new();

        // requestWillBeSent — url + method, and a new request goes in flight
        {
            let mut stream = page.event_listener::<EventRequestWillBeSent>().await?;
            let requests = requests.clone();
            let inflight = inflight.clone();
            tasks.push(tokio::spawn(async move {
                while let Some(ev) = stream.next().await {
                    // A redirect reuses the request id — count only the initial
                    // leg so the in-flight counter balances.
                    if ev.redirect_response.is_none() {
                        inflight.fetch_add(1, Ordering::Relaxed);
                    }
                    let id = ev.request_id.inner().clone();
                    let mut map = requests.lock().await;
                    let rec = map.entry(id.clone()).or_default();
                    rec.request_id = id;
                    rec.url = ev.request.url.clone();
                    rec.method = Some(ev.request.method.clone());
                }
            }));
        }

        // loadingFinished — a request leaves flight cleanly
        {
            let mut stream = page.event_listener::<EventLoadingFinished>().await?;
            let inflight = inflight.clone();
            tasks.push(tokio::spawn(async move {
                while stream.next().await.is_some() {
                    inflight.fetch_sub(1, Ordering::Relaxed);
                }
            }));
        }

        // loadingFailed — a request leaves flight with a transport error
        {
            let mut stream = page.event_listener::<EventLoadingFailed>().await?;
            let requests = requests.clone();
            let inflight = inflight.clone();
            tasks.push(tokio::spawn(async move {
                while let Some(ev) = stream.next().await {
                    inflight.fetch_sub(1, Ordering::Relaxed);
                    let id = ev.request_id.inner().clone();
                    let mut map = requests.lock().await;
                    let rec = map.entry(id.clone()).or_default();
                    rec.request_id = id;
                    rec.failure = Some(ev.error_text.clone());
                }
            }));
        }

        // responseReceived — status + mime + resource type
        {
            let mut stream = page.event_listener::<EventResponseReceived>().await?;
            let requests = requests.clone();
            tasks.push(tokio::spawn(async move {
                while let Some(ev) = stream.next().await {
                    let id = ev.request_id.inner().clone();
                    let mut map = requests.lock().await;
                    let rec = map.entry(id.clone()).or_default();
                    rec.request_id = id;
                    rec.url = ev.response.url.clone();
                    rec.status = Some(ev.response.status);
                    rec.mime_type = Some(ev.response.mime_type.clone());
                    rec.resource_type = Some(format!("{:?}", ev.r#type));
                }
            }));
        }

        Ok(Self {
            page: page.clone(),
            requests,
            inflight,
            tasks,
        })
    }

    /// Wait until no request has been in flight for `quiet`, or until `timeout`
    /// elapses. Returns `true` if the network went idle, `false` on timeout.
    pub async fn wait_for_idle(&self, quiet: Duration, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut idle_since: Option<Instant> = None;
        loop {
            if self.inflight.load(Ordering::Relaxed) <= 0 {
                let since = idle_since.get_or_insert_with(Instant::now);
                if since.elapsed() >= quiet {
                    return true;
                }
            } else {
                idle_since = None;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Fetch and store bodies for XHR/Fetch records we haven't bodied yet. Must
    /// run while the page that issued them is still current (Chrome drops the
    /// body on navigation).
    pub async fn hydrate_bodies(&self) {
        let targets: Vec<String> = {
            let map = self.requests.lock().await;
            map.values()
                .filter(|r| r.body_preview.is_none() && r.json.is_none() && is_bodyish(r))
                .map(|r| r.request_id.clone())
                .collect()
        };
        for id in targets {
            let params = GetResponseBodyParams::new(id.clone());
            let fetched =
                match tokio::time::timeout(BODY_FETCH_TIMEOUT, self.page.execute(params)).await {
                    Ok(Ok(resp)) => decode_body(&resp.result.body, resp.result.base64_encoded),
                    _ => None,
                };
            if let Some(text) = fetched {
                let mut map = self.requests.lock().await;
                if let Some(rec) = map.get_mut(&id) {
                    rec.json = serde_json::from_str(&text).ok();
                    rec.body_preview = Some(truncate(&text, BODY_PREVIEW_LEN));
                }
            }
        }
    }

    /// Records observed since the last `delta` call, advancing `seen` — so each
    /// task reports only the traffic it triggered.
    pub async fn delta(&self, seen: &mut HashSet<String>) -> Vec<NetworkRecord> {
        let map = self.requests.lock().await;
        let mut out = Vec::new();
        for (id, rec) in map.iter() {
            if seen.insert(id.clone()) {
                out.push(rec.clone());
            }
        }
        out
    }

    pub fn stop(&self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// Worth fetching a body for: XHR/Fetch responses with a status (skip the
/// document, images, scripts, fonts — we judge those by status alone).
fn is_bodyish(rec: &NetworkRecord) -> bool {
    if rec.status.is_none() {
        return false;
    }
    let rtype = rec.resource_type.as_deref().unwrap_or("");
    let mime = rec.mime_type.as_deref().unwrap_or("");
    matches!(rtype, "Xhr" | "Fetch") || mime.contains("json")
}

fn decode_body(body: &str, base64_encoded: bool) -> Option<String> {
    if base64_encoded {
        base64::engine::general_purpose::STANDARD
            .decode(body)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
    } else {
        Some(body.to_string())
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // Respect char boundaries.
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}
