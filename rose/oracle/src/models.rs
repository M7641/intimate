//! Domain types and the SSE event envelope.

use serde::{Deserialize, Serialize};

/// A piece of reading material in the pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub url: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub status: String,
    pub relevance: f32,
    pub ingested_at: String,
}

/// What the LLM judge decided about an item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub item_id: String,
    pub kept: bool,
    pub reason: String,
    pub lane: String,
}

/// One curated item as pushed to clients — the item plus why it surfaced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuratedItem {
    pub item: Item,
    pub reason: String,
}

/// The current learned taste, versioned so you can diff what the agent learned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasteProfile {
    pub version: i64,
    pub preamble: String,
    pub rationale: Option<String>,
    pub created_at: String,
}

/// Everything published over the `/feed` SSE stream.
///
/// `kind()` maps each variant to its SSE `event:` name so browser
/// `EventSource` handlers can subscribe per-type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeedEvent {
    /// Live lane: a single high-confidence item, streamed the moment it lands.
    Item(CuratedItem),
    /// Digest lane: a batch the judge ranked together.
    Digest { items: Vec<CuratedItem> },
    /// The taste profile was rewritten by the reflect loop.
    Profile { version: i64 },
    /// You gave feedback (echoed so connected clients stay in sync).
    Feedback { item_id: String, rating: i32 },
}

impl FeedEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            FeedEvent::Item(_) => "item",
            FeedEvent::Digest { .. } => "digest",
            FeedEvent::Profile { .. } => "profile",
            FeedEvent::Feedback { .. } => "feedback",
        }
    }
}
