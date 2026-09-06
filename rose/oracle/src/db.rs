//! SQLite persistence (sqlx) — same shape as the `nameless` pilot: a pool, WAL
//! mode, `sqlx::migrate!`, and small hand-written query helpers (no ORM).

use anyhow::Result;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use crate::embed;
use crate::models::{Item, TasteProfile};

pub async fn init_pool(db_path: &str) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite:{db_path}?mode=rwc"))
        .await?;

    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ── Items ───────────────────────────────────────────────

/// Insert a freshly-ingested item (status `pending`). Returns `None` if the URL
/// was already in the pool (deduplication on `url`).
#[allow(clippy::too_many_arguments)]
pub async fn insert_item(
    pool: &SqlitePool,
    url: &str,
    title: &str,
    summary: &str,
    content: &str,
    source: &str,
    embedding: &[f32],
) -> Result<Option<Item>> {
    let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM items WHERE url = ?")
        .bind(url)
        .fetch_optional(pool)
        .await?;
    if existing.is_some() {
        return Ok(None);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let ts = now();
    sqlx::query(
        "INSERT INTO items (id, url, title, summary, content, source, embedding, status, relevance, ingested_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', 0.0, ?)",
    )
    .bind(&id)
    .bind(url)
    .bind(title)
    .bind(summary)
    .bind(content)
    .bind(source)
    .bind(embed::encode(embedding))
    .bind(&ts)
    .execute(pool)
    .await?;

    Ok(Some(Item {
        id,
        url: url.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
        source: source.to_string(),
        status: "pending".to_string(),
        relevance: 0.0,
        ingested_at: ts,
    }))
}

pub async fn set_status(pool: &SqlitePool, item_id: &str, status: &str) -> Result<()> {
    sqlx::query("UPDATE items SET status = ? WHERE id = ?")
        .bind(status)
        .bind(item_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_relevance(pool: &SqlitePool, item_id: &str, relevance: f32) -> Result<()> {
    sqlx::query("UPDATE items SET relevance = ? WHERE id = ?")
        .bind(relevance)
        .bind(item_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// A row plus its decoded embedding — used by the scorer/curator.
pub struct ItemVec {
    pub item: Item,
    pub embedding: Vec<f32>,
}

pub async fn items_by_status(pool: &SqlitePool, status: &str) -> Result<Vec<ItemVec>> {
    let rows: Vec<(String, String, String, String, String, f32, String, String)> = sqlx::query_as(
        "SELECT id, url, title, summary, source, relevance, ingested_at, embedding
         FROM items WHERE status = ? ORDER BY ingested_at DESC",
    )
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(map_item_vec).collect())
}

pub async fn get_item(pool: &SqlitePool, item_id: &str) -> Result<Option<Item>> {
    let row: Option<(String, String, String, String, String, f32, String)> = sqlx::query_as(
        "SELECT id, url, title, summary, source, relevance, ingested_at FROM items WHERE id = ?",
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id, url, title, summary, source, relevance, ingested_at)| Item {
        id,
        url,
        title,
        summary,
        source,
        status: String::new(),
        relevance,
        ingested_at,
    }))
}

/// Items shown recently in the live lane — used for near-duplicate suppression.
pub async fn recent_live_vectors(pool: &SqlitePool, limit: u32) -> Result<Vec<Vec<f32>>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT embedding FROM items WHERE status = 'live' ORDER BY ingested_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(e,)| embed::decode(&e)).collect())
}

fn map_item_vec(
    (id, url, title, summary, source, relevance, ingested_at, embedding): (
        String,
        String,
        String,
        String,
        String,
        f32,
        String,
        String,
    ),
) -> ItemVec {
    ItemVec {
        embedding: embed::decode(&embedding),
        item: Item {
            id,
            url,
            title,
            summary,
            source,
            status: String::new(),
            relevance,
            ingested_at,
        },
    }
}

// ── Feedback (the fast loop's input) ─────────────────────

pub async fn insert_feedback(
    pool: &SqlitePool,
    item_id: &str,
    rating: i32,
    note: Option<&str>,
) -> Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO feedback (id, item_id, rating, note, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(item_id)
        .bind(rating)
        .bind(note)
        .bind(now())
        .execute(pool)
        .await?;
    Ok(())
}

/// All embeddings of items rated with the given sign (+1 liked / -1 disliked).
pub async fn feedback_vectors(pool: &SqlitePool, rating: i32) -> Result<Vec<Vec<f32>>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT i.embedding FROM items i
         JOIN feedback f ON f.item_id = i.id
         WHERE f.rating = ?",
    )
    .bind(rating)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(e,)| embed::decode(&e)).collect())
}

/// Recent feedback joined with item context — the rich signal `reflect` reads.
pub struct FeedbackRow {
    pub title: String,
    pub rating: i32,
    pub note: Option<String>,
}

pub async fn recent_feedback(pool: &SqlitePool, limit: u32) -> Result<Vec<FeedbackRow>> {
    let rows: Vec<(String, i32, Option<String>)> = sqlx::query_as(
        "SELECT i.title, f.rating, f.note FROM feedback f
         JOIN items i ON i.id = f.item_id
         ORDER BY f.created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(title, rating, note)| FeedbackRow { title, rating, note })
        .collect())
}

// ── Taste profile (the slow loop's output, versioned) ────

pub async fn current_profile(pool: &SqlitePool) -> Result<TasteProfile> {
    let row: Option<(i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT version, preamble, rationale, created_at FROM taste_profile
         ORDER BY version DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    if let Some((version, preamble, rationale, created_at)) = row {
        return Ok(TasteProfile {
            version,
            preamble,
            rationale,
            created_at,
        });
    }
    // Cold start: seed a generic curator persona.
    insert_profile(pool, SEED_PREAMBLE, Some("seed")).await
}

pub async fn insert_profile(
    pool: &SqlitePool,
    preamble: &str,
    rationale: Option<&str>,
) -> Result<TasteProfile> {
    let ts = now();
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO taste_profile (preamble, rationale, created_at) VALUES (?, ?, ?)
         RETURNING version",
    )
    .bind(preamble)
    .bind(rationale)
    .bind(&ts)
    .fetch_one(pool)
    .await?;
    Ok(TasteProfile {
        version: row.0,
        preamble: preamble.to_string(),
        rationale: rationale.map(str::to_string),
        created_at: ts,
    })
}

pub async fn profile_history(pool: &SqlitePool) -> Result<Vec<TasteProfile>> {
    let rows: Vec<(i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT version, preamble, rationale, created_at FROM taste_profile ORDER BY version DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(version, preamble, rationale, created_at)| TasteProfile {
            version,
            preamble,
            rationale,
            created_at,
        })
        .collect())
}

const SEED_PREAMBLE: &str = "You curate reading material for a software engineer. \
Favour technically substantive, advanced material: primary sources, papers, deep dives, \
and posts with real engineering detail. Reject shallow introductions, marketing, listicles, \
and anything the reader has likely already seen. You do not yet know the reader's specific \
interests — infer them from feedback over time.";

// ── Verdicts (the judge's decision log) ──────────────────

pub async fn insert_verdict(
    pool: &SqlitePool,
    item_id: &str,
    profile_version: i64,
    kept: bool,
    reason: &str,
    lane: &str,
) -> Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO verdicts (id, item_id, profile_version, kept, reason, lane, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(item_id)
    .bind(profile_version)
    .bind(kept as i32)
    .bind(reason)
    .bind(lane)
    .bind(now())
    .execute(pool)
    .await?;
    Ok(())
}

// ── Sources ──────────────────────────────────────────────

pub async fn insert_source(pool: &SqlitePool, kind: &str, url: &str) -> Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT OR IGNORE INTO sources (id, kind, url, added_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(kind)
        .bind(url)
        .bind(now())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_feed_sources(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT url FROM sources WHERE kind = 'rss' ORDER BY added_at")
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}
