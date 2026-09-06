//! chromiumoxide launch + lifecycle.
//!
//! `Browser::launch` returns the browser handle plus a `Handler` future that
//! must be driven for anything to happen — we spawn it onto a task and keep the
//! join handle so we can abort it on close.

use anyhow::{anyhow, Result};
use chromiumoxide::{Browser, BrowserConfig};
use futures::StreamExt;
use tokio::task::JoinHandle;

pub struct Session {
    pub browser: Browser,
    handler: JoinHandle<()>,
}

/// Launch a headless Chrome at the default capture viewport.
pub async fn launch() -> Result<Session> {
    let config = BrowserConfig::builder()
        .window_size(1280, 900)
        .build()
        .map_err(|e| anyhow!("browser config: {e}"))?;

    let (browser, mut handler) = Browser::launch(config).await?;

    // Drive the CDP event loop. Without this, no commands or events resolve.
    let handler = tokio::spawn(async move { while handler.next().await.is_some() {} });

    Ok(Session { browser, handler })
}

impl Session {
    pub async fn close(mut self) -> Result<()> {
        let _ = self.browser.close().await;
        self.handler.abort();
        Ok(())
    }
}
