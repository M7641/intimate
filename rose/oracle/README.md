# oracle

A Rust agent whose sole job is to **collect reading material** and **get better
at it from your feedback**. Lots of raw context flows in from many sources; an
LLM that has learned your taste decides what's actually worth your time.

The product is not the ingestion — that's a commodity. The product is the
**curation layer**.

```
   SOURCES (broad, cheap)               CURATION (the product)              SERVE
 ┌─────────────────────────┐      ┌──────────────────────────────┐   ┌──────────────┐
 │ RSS / Atom feeds        │      │ 1. embedding pre-filter       │   │ Axum + SSE   │
 │ dropped URLs            │ ───► │    (liked − disliked centroid)│──►│ /feed live   │
 └─────────────────────────┘      │ 2. LLM judge (the decider)    │   └──────┬───────┘
            │                     │    taste profile in preamble  │          │ you rate
            ▼                     └──────────────────────────────┘          │ 👍/👎 + note
     raw pool (SQLite)                       ▲                               ▼
                           taste_profile ────┴──◄── POST /feedback ──► fast loop (vectors)
                                              ▲
                                    POST /reflect ──► LLM rewrites the profile (slow loop)
```

## The two feedback loops

The whole design rests on running learning at **two speeds**, with no model
training and no fine-tuning:

- **Fast loop — embedding centroids.** Every item is embedded. New items are
  scored by `cosine(item, liked-centroid) − cosine(item, disliked-centroid)`,
  recomputed from the feedback table on every pass. The instant you rate
  something, the next item's score reflects it. Cheap, deterministic, no tokens.

- **Slow loop — `reflect`.** The centroid knows *"this looks like things you
  liked"*. It can't know *"you wanted papers, not blog posts"* — that intent is
  in your free-text **notes**. `POST /reflect` reads recent feedback and has the
  LLM rewrite the **taste profile** that the judge runs under. Each rewrite is a
  new **versioned** row, so you can diff what the agent learned and roll back.

## Hybrid curation (two lanes)

- **Live lane** (per item, embedding-only): a new item that strongly matches your
  demonstrated taste — and isn't a near-duplicate of something just shown — is
  streamed to `/feed` immediately. No LLM, no latency. On a cold start (no
  feedback yet) nothing fires here; everything defers to the judged digest.
- **Digest lane** (batch, LLM-judged): the pending pool is pre-filtered to a
  shortlist by the same centroid score, then the **whole shortlist** is judged at
  once so the LLM can decide *relatively* — rank, drop near-duplicates, and
  respect the learned profile. This is where "an LLM decides what's worth
  reading" actually happens. Runs on a timer and on demand via `POST /digest`.

## Run

```bash
cargo run
# listening on http://127.0.0.1:7878
# SSE feed: curl -N http://127.0.0.1:7878/feed
```

The LLM judge uses the same `auto_client` as the `nameless` pilot:
`ANTHROPIC_API_KEY` set → Anthropic API, otherwise the `claude` CLI. If neither
is available the digest falls back to a similarity heuristic so it never stalls.

### Quick tour

```bash
# 1. watch the live feed in one terminal
curl -N http://127.0.0.1:7878/feed

# 2. ingest some context (another terminal)
curl -X POST localhost:7878/ingest -d '{"url":"https://blog.rust-lang.org/"}'
curl -X POST localhost:7878/ingest -d '{"feed":"https://this-week-in-rust.org/atom.xml"}'

# 3. let the digest judge the pool (or wait for the timer)
curl -X POST localhost:7878/digest

# 4. teach it your taste
curl -X POST localhost:7878/feedback \
  -d '{"item_id":"<id>","rating":1,"note":"deep systems content, exactly this"}'

# 5. have it rewrite its own taste profile from your feedback
curl -X POST localhost:7878/reflect
curl localhost:7878/profile           # see current + full version history
```

## API

| Method & path        | Purpose                                                        |
|----------------------|----------------------------------------------------------------|
| `GET  /feed`         | SSE stream: `item` (live), `digest`, `profile`, `feedback`     |
| `GET  /items?status` | List items by `status` (pending/live/digested/rejected)        |
| `GET  /items/{id}`   | One item                                                       |
| `POST /ingest`       | Body `{url}` or `{feed}` (+ optional `source`)                 |
| `POST /sources`      | Register an RSS feed `{url}` for polling                       |
| `POST /digest`       | Run the digest lane now                                        |
| `POST /feedback`     | `{item_id, rating: -1|1, note?}` — the fast loop's input       |
| `POST /reflect`      | Rewrite the taste profile from recent feedback (slow loop)     |
| `GET  /profile`      | Current taste profile + versioned history                     |

## Configuration (env)

| Var | Default | Meaning |
|-----|---------|---------|
| `ORACLE_BIND` | `127.0.0.1:7878` | HTTP/SSE bind address |
| `ORACLE_DB` | `oracle.db` | SQLite path |
| `ORACLE_EMBED_DIM` | `256` | Embedding dimensionality |
| `ORACLE_DIGEST_INTERVAL` | `120` | Seconds between background digests |
| `ORACLE_DIGEST_SHORTLIST` | `20` | Max items sent to the judge per batch (cost cap) |
| `ORACLE_LIVE_THRESHOLD` | `0.35` | Min learned relevance to stream live |
| `ORACLE_DUP_THRESHOLD` | `0.92` | Cosine above which a live item is a near-dup |

## Pilot scope & the one upgrade that matters

The embedder is a **dependency-free lexical** one (hashed bag-of-tokens), so
oracle runs fully offline with zero API keys. It captures topical/keyword
overlap, not deep semantics. It lives behind the `Embedder` trait in
`src/embed.rs` — swapping in a real embedding model (e.g. a rig embedding
provider) is a one-file change and touches none of the curation logic, because
the centroid-scoring architecture is identical regardless of vector quality.

Text extraction is likewise a naive tag-stripper (`src/ingest.rs`); a readability
crate can replace it without affecting anything downstream.

## Layout

```
src/
├── main.rs      # wiring + background digest timer + graceful shutdown
├── server.rs    # Axum router, SSE /feed, AppState + broadcast channel
├── ingest.rs    # fetch + extract + RSS  (the commodity)
├── curate.rs    # live lane + digest lane + LLM judge  (the product)
├── embed.rs     # Embedder trait, lexical embedder, centroid relevance (fast loop)
├── profile.rs   # reflect: rewrite the versioned taste profile (slow loop)
├── llm.rs       # pluggable judge (claude CLI / Anthropic API), from `nameless`
├── db.rs        # sqlx/SQLite CRUD
├── models.rs    # domain types + SSE FeedEvent envelope
└── config.rs    # env-sourced config
```

Reuses patterns from sibling pilots: SSE + broadcast from `flow`, the pluggable
LLM `auto_client` + SQLite/sqlx setup from `nameless`.
