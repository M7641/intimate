//! HTTP + SSE surface. Reuses the `flow` pilot's shape: a broadcast channel that
//! the ingestion/curation side publishes to, and an SSE endpoint that subscribes
//! each connection via `BroadcastStream`.

use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::config::Config;
use crate::db;
use crate::embed::Embedder;
use crate::llm::LlmClient;
use crate::models::FeedEvent;
use crate::{curate, ingest, profile};

/// Shared, cheaply-cloneable application state (axum clones it per request).
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub config: Config,
    pub embedder: Arc<dyn Embedder>,
    pub llm: Arc<dyn LlmClient>,
    tx: broadcast::Sender<FeedEvent>,
}

impl AppState {
    pub fn new(
        pool: sqlx::SqlitePool,
        config: Config,
        embedder: Arc<dyn Embedder>,
        llm: Arc<dyn LlmClient>,
    ) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            pool,
            config,
            embedder,
            llm,
            tx,
        }
    }

    /// Publish to every connected `/feed` subscriber. No receivers → no-op.
    pub fn broadcast(&self, event: FeedEvent) {
        let _ = self.tx.send(event);
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/feed", get(feed))
        .route("/items", get(list_items))
        .route("/items/{id}", get(get_item))
        .route("/ingest", post(ingest_handler))
        .route("/sources", post(add_source))
        .route("/digest", post(digest_handler))
        .route("/feedback", post(feedback_handler))
        .route("/reflect", post(reflect_handler))
        .route("/profile", get(profile_handler))
        .with_state(state)
}

// ── SSE feed ─────────────────────────────────────────────

async fn feed(State(state): State<AppState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(ev) => {
                let data = serde_json::to_string(&ev).unwrap_or_default();
                Some(Ok(Event::default().event(ev.kind()).data(data)))
            }
            // Slow client lagged past the buffer — drop the gap, keep the stream.
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ── Items ────────────────────────────────────────────────

#[derive(Deserialize)]
struct StatusQuery {
    #[serde(default = "default_status")]
    status: String,
}

fn default_status() -> String {
    "digested".to_string()
}

async fn list_items(
    State(state): State<AppState>,
    Query(q): Query<StatusQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let items = db::items_by_status(&state.pool, &q.status).await?;
    let view: Vec<_> = items.into_iter().map(|iv| iv.item).collect();
    Ok(Json(serde_json::json!({ "items": view })))
}

async fn get_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match db::get_item(&state.pool, &id).await? {
        Some(item) => Ok(Json(serde_json::json!({ "item": item }))),
        None => Err(AppError::not_found("item not found")),
    }
}

// ── Ingestion ────────────────────────────────────────────

#[derive(Deserialize)]
struct IngestRequest {
    url: Option<String>,
    feed: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

async fn ingest_handler(
    State(state): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if let Some(feed) = req.feed {
        let added = ingest::ingest_feed(&state, &feed).await?;
        return Ok(Json(serde_json::json!({ "ingested": added, "feed": feed })));
    }
    if let Some(url) = req.url {
        let source = req.source.unwrap_or_else(|| "manual".to_string());
        let added = ingest::ingest_url(&state, &url, &source).await?;
        return Ok(Json(serde_json::json!({ "added": added, "url": url })));
    }
    Err(AppError::bad_request("provide `url` or `feed`"))
}

#[derive(Deserialize)]
struct SourceRequest {
    url: String,
}

async fn add_source(
    State(state): State<AppState>,
    Json(req): Json<SourceRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    db::insert_source(&state.pool, "rss", &req.url).await?;
    Ok(Json(serde_json::json!({ "added": req.url })))
}

// ── Curation triggers ────────────────────────────────────

async fn digest_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let kept = curate::run_digest(&state).await?;
    Ok(Json(serde_json::json!({ "kept": kept })))
}

// ── Feedback loops ───────────────────────────────────────

#[derive(Deserialize)]
struct FeedbackRequest {
    item_id: String,
    rating: i32,
    #[serde(default)]
    note: Option<String>,
}

async fn feedback_handler(
    State(state): State<AppState>,
    Json(req): Json<FeedbackRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let rating = req.rating.signum(); // clamp to -1 / 0 / 1
    if rating == 0 {
        return Err(AppError::bad_request("rating must be negative or positive"));
    }
    db::insert_feedback(&state.pool, &req.item_id, rating, req.note.as_deref()).await?;
    state.broadcast(FeedEvent::Feedback {
        item_id: req.item_id.clone(),
        rating,
    });
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn reflect_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let profile = profile::reflect(&state).await?;
    Ok(Json(serde_json::json!({
        "version": profile.version,
        "preamble": profile.preamble,
        "rationale": profile.rationale,
    })))
}

async fn profile_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let current = db::current_profile(&state.pool).await?;
    let history = db::profile_history(&state.pool).await?;
    Ok(Json(serde_json::json!({ "current": current, "history": history })))
}

// ── Error plumbing ───────────────────────────────────────

/// Maps internal errors to HTTP responses so handlers can use `?`.
pub struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn bad_request(msg: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.to_string(),
        }
    }
    fn not_found(msg: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.to_string(),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.message }))).into_response()
    }
}
