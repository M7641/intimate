//! Curation — the product. Two lanes decide what's worth your attention.
//!
//! * **Live lane** ([`try_live_lane`]): per-item, embedding-only. A new item that
//!   strongly matches your demonstrated taste (and isn't a near-duplicate of
//!   something just shown) is streamed immediately. No LLM, no latency, no cost.
//!
//! * **Digest lane** ([`run_digest`]): batch, LLM-judged. The pending pool is
//!   pre-filtered to a shortlist by the same centroid score, then the *whole
//!   shortlist* is handed to the LLM at once so it can judge **relatively** —
//!   rank, deduplicate by theme, and respect the learned taste profile. This is
//!   where "an LLM decides what's worth reading" actually happens.
//!
//! Both lanes share one learned signal (the feedback centroids) but spend it
//! differently: the live lane trades the LLM's comparative judgement for speed on
//! obvious wins; the digest keeps that judgement for everything ambiguous.

use anyhow::Result;
use serde::Deserialize;

use crate::db::{self, ItemVec};
use crate::embed::{self, centroid, relevance};
use crate::models::{CuratedItem, FeedEvent, Item};
use crate::server::AppState;

/// Recompute the liked/disliked centroids from the feedback table. Cheap enough
/// to do per scoring pass — this is what makes the fast loop feel instant.
async fn taste_centroids(state: &AppState) -> Result<(Option<Vec<f32>>, Option<Vec<f32>>)> {
    let dim = state.embedder.dim();
    let liked = db::feedback_vectors(&state.pool, 1).await?;
    let disliked = db::feedback_vectors(&state.pool, -1).await?;
    Ok((centroid(&liked, dim), centroid(&disliked, dim)))
}

/// Live lane: stream a single item now if it clears the bar and isn't a dup.
pub async fn try_live_lane(state: &AppState, item: &Item, vec: &[f32]) -> Result<()> {
    let (liked, disliked) = taste_centroids(state).await?;
    let score = relevance(vec, liked.as_ref(), disliked.as_ref());
    db::set_relevance(&state.pool, &item.id, score).await?;

    // Cold start (no liked centroid yet) → defer everything to the judged digest.
    if liked.is_none() || score < state.config.live_threshold {
        return Ok(());
    }

    // Near-duplicate suppression against what we streamed recently.
    let recent = db::recent_live_vectors(&state.pool, 30).await?;
    let is_dup = recent
        .iter()
        .any(|r| embed::cosine(vec, r) >= state.config.dup_threshold);
    if is_dup {
        return Ok(());
    }

    let reason = format!("strong match to your taste (score {score:.2})");
    let profile = db::current_profile(&state.pool).await?;
    db::set_status(&state.pool, &item.id, "live").await?;
    db::insert_verdict(&state.pool, &item.id, profile.version, true, &reason, "live").await?;

    let mut item = item.clone();
    item.status = "live".to_string();
    item.relevance = score;
    state.broadcast(FeedEvent::Item(CuratedItem { item, reason }));
    Ok(())
}

/// Digest lane: pre-filter the pending pool, let the LLM judge the shortlist as a
/// batch, persist verdicts, and broadcast the kept items as one digest event.
pub async fn run_digest(state: &AppState) -> Result<usize> {
    let pending = db::items_by_status(&state.pool, "pending").await?;
    if pending.is_empty() {
        return Ok(0);
    }

    let (liked, disliked) = taste_centroids(state).await?;

    // Pre-filter: score every pending item, keep the top N for the LLM.
    let mut scored: Vec<(f32, ItemVec)> = pending
        .into_iter()
        .map(|iv| {
            let s = relevance(&iv.embedding, liked.as_ref(), disliked.as_ref());
            (s, iv)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let shortlist: Vec<(f32, ItemVec)> =
        scored.into_iter().take(state.config.digest_shortlist).collect();

    for (s, iv) in &shortlist {
        db::set_relevance(&state.pool, &iv.item.id, *s).await?;
    }

    let profile = db::current_profile(&state.pool).await?;
    let verdicts = judge_batch(state, &profile.preamble, &shortlist).await;

    // Persist verdicts + statuses, collect the kept items for the digest event.
    let mut kept: Vec<CuratedItem> = Vec::new();
    for (idx, (score, iv)) in shortlist.iter().enumerate() {
        let v = verdicts.get(&idx);
        let keep = v.map(|v| v.keep).unwrap_or(false);
        let reason = v
            .map(|v| v.reason.clone())
            .unwrap_or_else(|| "no verdict returned".to_string());

        db::insert_verdict(&state.pool, &iv.item.id, profile.version, keep, &reason, "digest")
            .await?;
        db::set_status(&state.pool, &iv.item.id, if keep { "digested" } else { "rejected" })
            .await?;

        if keep {
            let mut item = iv.item.clone();
            item.status = "digested".to_string();
            item.relevance = *score;
            kept.push(CuratedItem { item, reason });
        }
    }

    // Items below the shortlist cut stay `pending` on purpose: as your taste
    // sharpens, a later digest may surface one that didn't make the cut today.

    let n = kept.len();
    if !kept.is_empty() {
        state.broadcast(FeedEvent::Digest { items: kept });
    }
    Ok(n)
}

// ── LLM judging ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct JudgeVerdict {
    /// 0-based index into the shortlist we sent.
    index: usize,
    keep: bool,
    reason: String,
}

struct ParsedVerdict {
    keep: bool,
    reason: String,
}

/// Ask the LLM to judge the whole shortlist at once. Falls back to a relevance
/// heuristic if the model is unavailable or returns unparseable output, so the
/// digest never silently stalls.
async fn judge_batch(
    state: &AppState,
    preamble: &str,
    shortlist: &[(f32, ItemVec)],
) -> std::collections::HashMap<usize, ParsedVerdict> {
    let mut listing = String::new();
    for (i, (score, iv)) in shortlist.iter().enumerate() {
        listing.push_str(&format!(
            "[{i}] (similarity {score:.2}) {}\n    {}\n    {}\n\n",
            iv.item.title, iv.item.url, iv.item.summary
        ));
    }

    let system = format!(
        "{preamble}\n\nYou are judging a batch of candidate articles together. \
Because you see them all at once, judge RELATIVELY: prefer the most substantive, \
drop near-duplicates of a better entry in the same batch, and drop anything that \
doesn't fit the reader's taste. Keep only what is genuinely worth their time."
    );
    let user = format!(
        "Candidates:\n\n{listing}\nReturn ONLY a JSON array, one object per candidate you \
have an opinion on, like:\n[{{\"index\": 0, \"keep\": true, \"reason\": \"...\"}}]\n\
Keep reasons to one short sentence. Be selective — keeping everything is a failure."
    );

    match state.llm.prompt(&system, &user).await {
        Ok(text) => parse_verdicts(&text).unwrap_or_else(|| heuristic(shortlist)),
        Err(e) => {
            tracing::warn!("judge unavailable ({e}); falling back to relevance heuristic");
            heuristic(shortlist)
        }
    }
}

/// Extract the JSON array from a possibly-chatty LLM response.
fn parse_verdicts(text: &str) -> Option<std::collections::HashMap<usize, ParsedVerdict>> {
    let start = text.find('[')?;
    let end = text.rfind(']')? + 1;
    let json = &text[start..end];
    let parsed: Vec<JudgeVerdict> = serde_json::from_str(json).ok()?;
    Some(
        parsed
            .into_iter()
            .map(|v| {
                (
                    v.index,
                    ParsedVerdict {
                        keep: v.keep,
                        reason: v.reason,
                    },
                )
            })
            .collect(),
    )
}

/// No LLM? Keep the top third of the shortlist by similarity.
fn heuristic(shortlist: &[(f32, ItemVec)]) -> std::collections::HashMap<usize, ParsedVerdict> {
    let cutoff = (shortlist.len() / 3).max(1);
    shortlist
        .iter()
        .enumerate()
        .map(|(i, _)| {
            (
                i,
                ParsedVerdict {
                    keep: i < cutoff,
                    reason: "ranked by similarity (LLM judge unavailable)".to_string(),
                },
            )
        })
        .collect()
}
