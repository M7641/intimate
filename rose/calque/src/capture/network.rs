//! Network capture over the CDP `Network` domain.
//!
//! We subscribe to events *before* navigating, then correlate `requestWillBeSent`
//! (url + method) with `responseReceived` (status + mime) by request id, so each
//! observed request becomes one complete [`NetworkRecord`]. WebSocket frames are
//! collected separately — that channel carries the reactive protocol for apps
//! like R Shiny.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chromiumoxide::cdp::browser_protocol::network::{
    EnableParams, EventLoadingFailed, EventLoadingFinished, EventRequestWillBeSent,
    EventResponseReceived, EventWebSocketFrameReceived, EventWebSocketFrameSent,
};
use chromiumoxide::Page;
use futures::StreamExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::models::{NetworkRecord, WsDirection, WsFrame};

const PAYLOAD_PREVIEW_LEN: usize = 512;

pub struct NetworkCapture {
    requests: Arc<Mutex<HashMap<String, NetworkRecord>>>,
    websocket: Arc<Mutex<Vec<WsFrame>>>,
    /// HTTP requests currently in flight — drives [`wait_for_idle`].
    inflight: Arc<AtomicI64>,
    tasks: Vec<JoinHandle<()>>,
}

impl NetworkCapture {
    /// Enable the Network domain and start listening. Call before `goto`.
    pub async fn start(page: &Page) -> Result<Self> {
        page.execute(EnableParams::default()).await?;

        let requests: Arc<Mutex<HashMap<String, NetworkRecord>>> = Arc::default();
        let websocket: Arc<Mutex<Vec<WsFrame>>> = Arc::default();
        let inflight: Arc<AtomicI64> = Arc::default();
        let mut tasks = Vec::new();

        // requestWillBeSent — url + method, and a new request goes in flight
        {
            let mut stream = page.event_listener::<EventRequestWillBeSent>().await?;
            let requests = requests.clone();
            let inflight = inflight.clone();
            tasks.push(tokio::spawn(async move {
                while let Some(ev) = stream.next().await {
                    // A redirect reuses the request id (its `redirect_response` is
                    // set) — count only the initial leg so the counter balances.
                    if ev.redirect_response.is_none() {
                        inflight.fetch_add(1, Ordering::Relaxed);
                    }
                    let mut map = requests.lock().await;
                    let rec = map.entry(ev.request_id.inner().clone()).or_default();
                    rec.url = ev.request.url.clone();
                    rec.method = Some(ev.request.method.clone());
                }
            }));
        }

        // loadingFinished / loadingFailed — a request leaves flight
        for kind in ["finished", "failed"] {
            let inflight = inflight.clone();
            if kind == "finished" {
                let mut stream = page.event_listener::<EventLoadingFinished>().await?;
                tasks.push(tokio::spawn(async move {
                    while stream.next().await.is_some() {
                        inflight.fetch_sub(1, Ordering::Relaxed);
                    }
                }));
            } else {
                let mut stream = page.event_listener::<EventLoadingFailed>().await?;
                tasks.push(tokio::spawn(async move {
                    while stream.next().await.is_some() {
                        inflight.fetch_sub(1, Ordering::Relaxed);
                    }
                }));
            }
        }

        // responseReceived — status + mime + resource type
        {
            let mut stream = page.event_listener::<EventResponseReceived>().await?;
            let requests = requests.clone();
            tasks.push(tokio::spawn(async move {
                while let Some(ev) = stream.next().await {
                    let mut map = requests.lock().await;
                    let rec = map.entry(ev.request_id.inner().clone()).or_default();
                    rec.url = ev.response.url.clone();
                    rec.status = Some(ev.response.status);
                    rec.mime_type = Some(ev.response.mime_type.clone());
                    rec.resource_type = Some(format!("{:?}", ev.r#type));
                }
            }));
        }

        // WebSocket frames — the reactive channel (Shiny inputs/outputs)
        {
            let mut stream = page.event_listener::<EventWebSocketFrameReceived>().await?;
            let websocket = websocket.clone();
            tasks.push(tokio::spawn(async move {
                while let Some(ev) = stream.next().await {
                    websocket.lock().await.push(WsFrame {
                        request_id: ev.request_id.inner().clone(),
                        direction: WsDirection::Received,
                        opcode: ev.response.opcode,
                        payload_preview: preview(&ev.response.payload_data),
                    });
                }
            }));
        }
        {
            let mut stream = page.event_listener::<EventWebSocketFrameSent>().await?;
            let websocket = websocket.clone();
            tasks.push(tokio::spawn(async move {
                while let Some(ev) = stream.next().await {
                    websocket.lock().await.push(WsFrame {
                        request_id: ev.request_id.inner().clone(),
                        direction: WsDirection::Sent,
                        opcode: ev.response.opcode,
                        payload_preview: preview(&ev.response.payload_data),
                    });
                }
            }));
        }

        Ok(Self {
            requests,
            websocket,
            inflight,
            tasks,
        })
    }

    /// Wait until no HTTP request has been in flight for `quiet`, or until
    /// `timeout` elapses. Returns `true` if the network went idle, `false` on
    /// timeout. Lets a screenshot wait for the page's data to actually arrive
    /// instead of guessing with a fixed sleep.
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

    /// Records observed since the last `delta` call, advancing the cursors.
    ///
    /// `seen` tracks request ids already attributed; `ws_cursor` tracks how many
    /// WebSocket frames have been attributed. Used by the recipe engine to
    /// attribute traffic to the interaction step that triggered it.
    pub async fn delta(
        &self,
        seen: &mut HashSet<String>,
        ws_cursor: &mut usize,
    ) -> (Vec<NetworkRecord>, Vec<WsFrame>) {
        let map = self.requests.lock().await;
        let mut network_delta = Vec::new();
        for (id, rec) in map.iter() {
            if seen.insert(id.clone()) {
                network_delta.push(rec.clone());
            }
        }
        let ws = self.websocket.lock().await;
        let ws_delta = ws
            .get(*ws_cursor..)
            .map(<[WsFrame]>::to_vec)
            .unwrap_or_default();
        *ws_cursor = ws.len();
        (network_delta, ws_delta)
    }

    /// Stop listening and return everything observed.
    pub async fn finish(self) -> (Vec<NetworkRecord>, Vec<WsFrame>) {
        let network = self.requests.lock().await.values().cloned().collect();
        let websocket = self.websocket.lock().await.clone();
        for task in &self.tasks {
            task.abort();
        }
        (network, websocket)
    }
}

fn preview(payload: &str) -> String {
    if payload.len() <= PAYLOAD_PREVIEW_LEN {
        payload.to_string()
    } else {
        format!("{}…", &payload[..PAYLOAD_PREVIEW_LEN])
    }
}
