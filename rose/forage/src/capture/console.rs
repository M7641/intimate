//! Console + uncaught-exception capture over the CDP `Runtime` domain.
//!
//! Mirrors the network listener pattern: subscribe before navigating, push
//! entries into a shared buffer, expose `delta` so the scheduler attributes
//! messages to the task that triggered them. We keep only errors, warnings, and
//! uncaught exceptions — those are the ones that signal broken functionality.

use std::sync::Arc;

use anyhow::Result;
use chromiumoxide::cdp::js_protocol::runtime::{
    ConsoleApiCalledType, EnableParams, EventConsoleApiCalled, EventExceptionThrown, RemoteObject,
};
use chromiumoxide::Page;
use futures::StreamExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    /// `error`, `warning`, or `exception`.
    pub level: String,
    pub text: String,
}

impl ConsoleEntry {
    pub fn is_error(&self) -> bool {
        self.level == "error" || self.level == "exception"
    }
}

pub struct ConsoleCapture {
    entries: Arc<Mutex<Vec<ConsoleEntry>>>,
    tasks: Vec<JoinHandle<()>>,
}

impl ConsoleCapture {
    pub async fn start(page: &Page) -> Result<Self> {
        page.execute(EnableParams::default()).await?;

        let entries: Arc<Mutex<Vec<ConsoleEntry>>> = Arc::default();
        let mut tasks = Vec::new();

        // console.error / console.warn
        {
            let mut stream = page.event_listener::<EventConsoleApiCalled>().await?;
            let entries = entries.clone();
            tasks.push(tokio::spawn(async move {
                while let Some(ev) = stream.next().await {
                    let level = match ev.r#type {
                        ConsoleApiCalledType::Error => "error",
                        ConsoleApiCalledType::Warning => "warning",
                        _ => continue,
                    };
                    let text = ev
                        .args
                        .iter()
                        .filter_map(arg_text)
                        .collect::<Vec<_>>()
                        .join(" ");
                    entries.lock().await.push(ConsoleEntry {
                        level: level.to_string(),
                        text,
                    });
                }
            }));
        }

        // Uncaught exceptions
        {
            let mut stream = page.event_listener::<EventExceptionThrown>().await?;
            let entries = entries.clone();
            tasks.push(tokio::spawn(async move {
                while let Some(ev) = stream.next().await {
                    let details = &ev.exception_details;
                    let mut text = details.text.clone();
                    if let Some(exc) = &details.exception {
                        if let Some(desc) = &exc.description {
                            text = format!("{text}: {desc}");
                        }
                    }
                    entries.lock().await.push(ConsoleEntry {
                        level: "exception".to_string(),
                        text,
                    });
                }
            }));
        }

        Ok(Self { entries, tasks })
    }

    /// Entries observed since the last `delta`, advancing `cursor`.
    pub async fn delta(&self, cursor: &mut usize) -> Vec<ConsoleEntry> {
        let v = self.entries.lock().await;
        let out = v
            .get(*cursor..)
            .map(<[ConsoleEntry]>::to_vec)
            .unwrap_or_default();
        *cursor = v.len();
        out
    }

    pub fn stop(&self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// Pull readable text out of a console argument.
fn arg_text(o: &RemoteObject) -> Option<String> {
    if let Some(v) = &o.value {
        return Some(match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        });
    }
    o.description.clone()
}
