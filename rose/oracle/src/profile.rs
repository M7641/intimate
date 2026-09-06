//! The reflect loop — the *slow* feedback loop.
//!
//! The fast loop (centroids in `embed`/`curate`) learns "this looks like things
//! you liked". It cannot learn "you said you wanted papers, not blog posts" or
//! "you're tired of beginner Tokio content" — that intent lives in your free-text
//! notes. `reflect` reads the recent feedback, especially the notes, and has the
//! LLM rewrite the taste profile preamble that the judge runs under.
//!
//! Each rewrite is a new versioned row (never an overwrite), so you can diff what
//! the agent learned and roll back a regression.

use anyhow::Result;

use crate::db;
use crate::models::TasteProfile;
use crate::server::AppState;

/// Read recent feedback and produce a new taste-profile version. Returns the new
/// profile (or the unchanged current one if there's nothing to learn from).
pub async fn reflect(state: &AppState) -> Result<TasteProfile> {
    let feedback = db::recent_feedback(&state.pool, 50).await?;
    let current = db::current_profile(&state.pool).await?;

    if feedback.is_empty() {
        return Ok(current);
    }

    let mut signal = String::new();
    for f in &feedback {
        let mark = if f.rating > 0 { "LIKED" } else { "REJECTED" };
        let note = f.note.as_deref().unwrap_or("(no note)");
        signal.push_str(&format!("- {mark}: \"{}\" — {note}\n", f.title));
    }

    let system = "You maintain a concise 'taste profile' that instructs a reading-material \
curator. You are given the current profile and recent reader feedback. Rewrite the profile so \
the curator better matches what the reader actually wants. Keep it under 150 words, concrete, \
and focused on observable signals (topics, depth, formats, sources to prefer or avoid). Pay \
special attention to the free-text notes — they carry intent the ratings alone cannot. \
Return ONLY the new profile text, no preamble or explanation.";

    let user = format!(
        "CURRENT PROFILE:\n{}\n\nRECENT FEEDBACK:\n{}\n\nWrite the improved profile.",
        current.preamble, signal
    );

    let new_preamble = match state.llm.prompt(system, &user).await {
        Ok(text) if !text.trim().is_empty() => text.trim().to_string(),
        Ok(_) => return Ok(current),
        Err(e) => {
            tracing::warn!("reflect: LLM unavailable ({e}); keeping current profile");
            return Ok(current);
        }
    };

    let rationale = format!("reflected over {} feedback item(s)", feedback.len());
    let profile = db::insert_profile(&state.pool, &new_preamble, Some(&rationale)).await?;

    state.broadcast(crate::models::FeedEvent::Profile {
        version: profile.version,
    });
    Ok(profile)
}
