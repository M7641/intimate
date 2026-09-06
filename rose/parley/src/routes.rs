//! HTTP handlers.
//!
//! `/api/converse` is the full voice loop (audio in, audio out). `/api/chat` is a
//! text-only shortcut for testing the conversation with curl, skipping ASR/TTS.

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{Level, Turn};
use crate::memory::{self, VocabItem};
use crate::state::AppState;
use crate::areas;

/// What the client gets back from a conversation turn.
#[derive(Serialize)]
pub struct ConverseResponse {
    /// The session id to send on the next turn (echoed or freshly minted).
    pub session: Uuid,
    /// What the learner was heard to say (from ASR).
    pub transcript: String,
    /// The tutor's reply text.
    pub reply: String,
    /// The tutor's reply as base64 WAV, or null in text mode.
    pub audio: Option<String>,
}

/// Turn any error into a 500 with a readable message. M1-simple; refine later.
fn err(e: impl std::fmt::Display) -> Response {
    tracing::error!(error = %e, "request failed");
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
}

/// Run the conversational core: append the learner turn, get a reply, append it.
/// Returns the reply text. Kept separate so both handlers share it.
///
/// Along the way it reads the learner's durable memories to *steer* the tutor
/// toward an unpracticed everyday area, and writes back the words the learner just
/// produced. A memory backend that is down never breaks the conversation — we log
/// and carry on with an empty profile.
async fn converse_core(state: &AppState, session: Uuid, learner_text: String) -> anyhow::Result<String> {
    // Words the learner produced this turn, captured before the text is moved.
    let words = areas::content_words(&learner_text);

    // Lock only to read/write history — never across an await.
    let history = {
        let mut sessions = state.sessions.lock().unwrap();
        let turns = sessions.entry(session).or_default();
        turns.push(Turn::learner(learner_text));
        turns.clone()
    };

    // Load memories to steer toward an area the learner has not yet practiced.
    // A load failure is non-fatal: fall back to a blank profile.
    let mut memories = state.memory.load(memory::DEFAULT_LEARNER).await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "memory load failed; using blank profile");
        crate::memory::Memories::empty(memory::DEFAULT_LEARNER, state.level)
    });
    let steer = areas::steer_line(&memories);

    let reply = state
        .conversant
        .reply(&state.system_prompt(steer.as_deref()), &history)
        .await?;

    state
        .sessions
        .lock()
        .unwrap()
        .entry(session)
        .or_default()
        .push(Turn::tutor(reply.clone()));

    // Fold the learner's new words into their memories and persist. A save failure
    // is non-fatal — the learner still gets their reply.
    memories.record_words(words);
    if let Err(e) = state.memory.save(&memories).await {
        tracing::warn!(error = %e, "memory save failed; profile not updated this turn");
    }

    Ok(reply)
}

/// `POST /api/converse` — multipart with fields `session` (optional uuid) and
/// `audio` (the recorded clip). The full voice loop.
pub async fn converse(State(state): State<AppState>, mut form: Multipart) -> Response {
    let mut session: Option<Uuid> = None;
    let mut audio: Vec<u8> = Vec::new();
    let mut mime = "audio/webm".to_string();

    while let Some(field) = match form.next_field().await {
        Ok(f) => f,
        Err(e) => return err(e),
    } {
        match field.name() {
            Some("session") => {
                let raw = field.text().await.unwrap_or_default();
                session = Uuid::parse_str(&raw).ok();
            }
            Some("audio") => {
                if let Some(ct) = field.content_type() {
                    mime = ct.to_string();
                }
                audio = match field.bytes().await {
                    Ok(b) => b.to_vec(),
                    Err(e) => return err(e),
                };
            }
            _ => {}
        }
    }

    let session = session.unwrap_or_else(Uuid::new_v4);

    let transcript = match state.transcriber.transcribe(&audio, &mime).await {
        Ok(t) => t,
        Err(e) => return err(e),
    };

    let reply = match converse_core(&state, session, transcript.clone()).await {
        Ok(r) => r,
        Err(e) => return err(e),
    };

    let audio_b64 = match state.synthesizer.synthesize(&reply).await {
        Ok(bytes) => Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
        Err(e) => return err(e),
    };

    Json(ConverseResponse { session, transcript, reply, audio: audio_b64 }).into_response()
}

/// Request body for the text-only endpoint.
#[derive(Deserialize)]
pub struct ChatRequest {
    pub session: Option<Uuid>,
    pub text: String,
}

/// `POST /api/chat` — JSON text in, text reply out. No audio. For quick testing.
pub async fn chat(State(state): State<AppState>, Json(req): Json<ChatRequest>) -> Response {
    let session = req.session.unwrap_or_else(Uuid::new_v4);
    match converse_core(&state, session, req.text).await {
        Ok(reply) => Json(ConverseResponse {
            session,
            transcript: String::new(),
            reply,
            audio: None,
        })
        .into_response(),
        Err(e) => err(e),
    }
}

/// What the Progress page reads: the learner reflected back to themselves.
#[derive(Serialize)]
pub struct ProgressResponse {
    /// The learner's current estimated level.
    pub level: Level,
    /// Distinct words the learner has produced across all conversations.
    pub distinct_words: usize,
    /// The most-used words, most frequent first (capped).
    pub top_words: Vec<VocabItem>,
    /// Everyday areas the learner has already talked about.
    pub practiced_areas: Vec<String>,
    /// The next area the tutor will steer toward, if any remain.
    pub next_area: Option<String>,
}

/// `GET /api/progress` — read the learner's durable memories for the Progress page.
/// Pure read of the memory store; no model runs.
pub async fn progress(State(state): State<AppState>) -> Response {
    let memories = match state.memory.load(memory::DEFAULT_LEARNER).await {
        Ok(m) => m,
        Err(e) => return err(e),
    };

    // Rank vocabulary by how often it has been used, then recency.
    let mut top_words = memories.vocab.clone();
    top_words.sort_by(|a, b| b.count.cmp(&a.count).then(b.last_seen.cmp(&a.last_seen)));
    top_words.truncate(20);

    Json(ProgressResponse {
        level: state.level,
        distinct_words: memories.distinct_word_count(),
        top_words,
        practiced_areas: areas::practiced_areas(&memories),
        next_area: areas::suggested_area(&memories),
    })
    .into_response()
}

/// Request body for saving a word.
#[derive(Deserialize)]
pub struct SaveWordRequest {
    pub word: String,
}

/// `POST /api/words` — save a word and collect its definition from the dictionary.
///
/// Saving is the learner's intent, so the word is stored even if the lookup fails
/// (offline, or not in the dictionary): it comes back with `definition: null` and
/// can be re-saved later to retry. Calling this again for a saved word refreshes
/// its definition.
pub async fn save_word(State(state): State<AppState>, Json(req): Json<SaveWordRequest>) -> Response {
    let word = req.word.trim().to_string();
    if word.is_empty() {
        return (StatusCode::BAD_REQUEST, "empty word").into_response();
    }

    // Best-effort definition: a lookup failure is not fatal to saving.
    let definition = match state.dictionary.define(&word).await {
        Ok(def) => Some(def),
        Err(e) => {
            tracing::warn!(%word, error = %e, "definition lookup failed; saving word without it");
            None
        }
    };

    let mut memories = state.memory.load(memory::DEFAULT_LEARNER).await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "memory load failed; using blank profile");
        crate::memory::Memories::empty(memory::DEFAULT_LEARNER, state.level)
    });
    let item = memories.save_word(&word, definition);
    if let Err(e) = state.memory.save(&memories).await {
        return err(e);
    }

    Json(item).into_response()
}

/// `GET /api/words` — the learner's saved words, with definitions, newest first.
/// Powers the Vocabulary page. Pure read of the memory store.
pub async fn list_words(State(state): State<AppState>) -> Response {
    match state.memory.load(memory::DEFAULT_LEARNER).await {
        Ok(memories) => Json(memories.saved_words()).into_response(),
        Err(e) => err(e),
    }
}

/// `GET /health` — liveness.
pub async fn health() -> &'static str {
    "ok"
}
