//! Ingestion: pull raw context from many sources into the pool.
//!
//! Ingestion is deliberately "dumb" — fetch, extract readable text, embed, store.
//! All the intelligence lives downstream in [`crate::curate`]. The point of the
//! product is the curation layer, so we make ingestion broad and cheap.

use anyhow::{Context, Result};

use crate::curate;
use crate::db;
use crate::server::AppState;

/// Fetch a single URL, extract its text, embed it, store it, and run the live
/// lane. Returns `true` if it was newly added (not a duplicate URL).
pub async fn ingest_url(state: &AppState, url: &str, source: &str) -> Result<bool> {
    let html = fetch(url).await?;
    let (title, content) = extract(&html, url);
    let summary = summarize(&content);

    let embedding = state.embedder.embed(&format!("{title}\n{summary}")).await?;

    let item = db::insert_item(
        &state.pool,
        url,
        &title,
        &summary,
        &content,
        source,
        &embedding,
    )
    .await?;

    match item {
        None => Ok(false), // duplicate URL, already in the pool
        Some(item) => {
            // Live lane: high-confidence matches stream immediately.
            curate::try_live_lane(state, &item, &embedding).await?;
            Ok(true)
        }
    }
}

/// Poll an RSS/Atom feed and ingest every entry's link. Returns count added.
pub async fn ingest_feed(state: &AppState, feed_url: &str) -> Result<usize> {
    let bytes = reqwest::get(feed_url)
        .await
        .with_context(|| format!("fetching feed {feed_url}"))?
        .bytes()
        .await?;

    let feed = feed_rs::parser::parse(&bytes[..])
        .with_context(|| format!("parsing feed {feed_url}"))?;

    let mut added = 0;
    for entry in feed.entries {
        let Some(link) = entry.links.first().map(|l| l.href.clone()) else {
            continue;
        };
        // Be resilient: one bad entry shouldn't abort the whole feed.
        match ingest_url(state, &link, feed_url).await {
            Ok(true) => added += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!("skipping {link}: {e}"),
        }
    }
    Ok(added)
}

async fn fetch(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("oracle/0.1 (reading-material curator)")
        .build()?;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("fetching {url}"))?;
    Ok(resp.text().await?)
}

/// Naive readable-text extraction: drop `<script>`/`<style>`, strip tags, pull
/// the `<title>`, collapse whitespace. Good enough for a pilot; swap in a real
/// readability crate later without touching anything upstream.
fn extract(html: &str, url: &str) -> (String, String) {
    let title = between(html, "<title>", "</title>")
        .map(decode_entities)
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| url.to_string());

    let body = strip_block(html, "script");
    let body = strip_block(&body, "style");
    let text = strip_tags(&body);
    let text = decode_entities(&text);
    let text = collapse_ws(&text);

    (title, text)
}

/// First ~600 chars of clean text — a cheap stand-in summary the judge reads.
fn summarize(content: &str) -> String {
    let s: String = content.chars().take(600).collect();
    if content.chars().count() > 600 {
        format!("{s}…")
    } else {
        s
    }
}

fn between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let s = haystack.to_lowercase();
    let i = s.find(start)? + start.len();
    let j = s[i..].find(end)? + i;
    Some(&haystack[i..j])
}

fn strip_block(html: &str, tag: &str) -> String {
    let lower = html.to_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some(rel) = lower[cursor..].find(&open) {
        let start = cursor + rel;
        out.push_str(&html[cursor..start]);
        match lower[start..].find(&close) {
            Some(rel_end) => cursor = start + rel_end + close.len(),
            None => {
                cursor = html.len();
                break;
            }
        }
    }
    out.push_str(&html[cursor..]);
    out
}

fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
