-- oracle: ingest → curate → serve.
-- The interesting state lives here: items (the raw pool + their embeddings),
-- feedback (your signal), taste_profile (what the agent has *learned*, versioned),
-- and verdicts (what the LLM judge decided, and why).

CREATE TABLE IF NOT EXISTS sources (
    id       TEXT PRIMARY KEY,
    kind     TEXT NOT NULL,            -- 'rss'
    url      TEXT NOT NULL UNIQUE,
    added_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS items (
    id          TEXT PRIMARY KEY,
    url         TEXT NOT NULL UNIQUE,
    title       TEXT NOT NULL,
    summary     TEXT NOT NULL,
    content     TEXT NOT NULL,
    source      TEXT NOT NULL,         -- 'manual' | feed url
    embedding   TEXT NOT NULL,         -- JSON array of f32 (lexical pilot embedder)
    status      TEXT NOT NULL DEFAULT 'pending',  -- pending | live | digested | rejected
    relevance   REAL NOT NULL DEFAULT 0.0,        -- learned centroid score at last scoring
    ingested_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS feedback (
    id         TEXT PRIMARY KEY,
    item_id    TEXT NOT NULL REFERENCES items(id),
    rating     INTEGER NOT NULL,       -- -1 (down) | 1 (up)
    note       TEXT,                   -- free-text: the rich signal the reflect loop reads
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS taste_profile (
    version    INTEGER PRIMARY KEY AUTOINCREMENT,
    preamble   TEXT NOT NULL,          -- injected into the judge's system prompt
    rationale  TEXT,                   -- why the reflect loop changed it (for diffing)
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS verdicts (
    id              TEXT PRIMARY KEY,
    item_id         TEXT NOT NULL REFERENCES items(id),
    profile_version INTEGER NOT NULL,
    kept            INTEGER NOT NULL,  -- 1 worth reading | 0 dropped
    reason          TEXT NOT NULL,
    lane            TEXT NOT NULL,     -- 'live' | 'digest'
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_items_status   ON items(status);
CREATE INDEX IF NOT EXISTS idx_feedback_item  ON feedback(item_id);
CREATE INDEX IF NOT EXISTS idx_verdicts_item  ON verdicts(item_id);
