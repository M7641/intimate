# ledger

Explain one SQL query against two engines at once and see where the cost goes:

- **DuckDB** — analytical, columnar, vectorized (think OLAP / warehouse).
- **Postgres** — transactional, row-store, index-driven (think OLTP).

You type a query, `ledger` runs `EXPLAIN` on both engines over the *same*
schema and data, normalizes the two very different plan formats into one tree,
and highlights the bottleneck and a few engine-specific cost hints.

The point is the contrast: a `Seq Scan` is an alarm in Postgres (read every
page — an index would help) but the normal, fast path in DuckDB (columnar
scans are what it is built for). ledger makes that difference concrete.

## Architecture

```
React app  ──POST /api/explain──▶  Axum server ──┬─▶ Engine: DuckDB   (embedded)
 (Vite)     ◀──── plans ─────────                 └─▶ Engine: Postgres (container)
```

One Axum service exposes every engine behind a single `Engine` trait
(`src/engine.rs`). Each backend folds its native `EXPLAIN` JSON into a shared
`PlanNode` tree (`src/plan.rs`). Adding **Redshift** or **Snowflake** later is
one more `impl Engine` plus one line in `build_registry` — nothing else moves.
That is why the two engines are not two separate services.

| File | Role |
| --- | --- |
| `src/engine.rs` | The `Engine` trait + registry — the extension seam |
| `src/plan.rs` | Normalized `PlanNode` tree, summary, cost heuristics |
| `src/engines/duckdb.rs` | `EXPLAIN (FORMAT json)` → normalized tree |
| `src/engines/postgres.rs` | `EXPLAIN (FORMAT JSON)` → normalized tree + OLTP hints |
| `src/api.rs` | `/api/explain`, `/api/engines`; serves built frontend |
| `seed/*.sql` | Identical schema + data in both engines |

## Why the Postgres container is "production-like"

The `postgres:17-alpine` image is small (~80 MB) but ships the **real**
planner. What makes a plan resemble a production server is not the image, it is:

1. **Statistics** — the seed ends with `ANALYZE`, so the planner has real
   row-count and distribution data instead of guesses.
2. **Cost knobs** — `docker-compose.yml` sets `random_page_cost`,
   `effective_cache_size`, `work_mem`, etc. to SSD-backed prod-ish values.
   These change *which plan wins*, so they live in version control.
3. **Indexes** — the seed builds the secondary indexes a real OLTP schema
   would have, giving the planner the option of index scans.

## Run it

Prerequisites: Rust, `docker` (or `podman`) + compose, and `bun` (or `npm`).

```bash
# 1. Start the seeded Postgres (first boot runs the seed + ANALYZE).
docker compose up -d

# 2. Start the API (embeds and seeds DuckDB in memory on boot).
cargo run                        # listens on 127.0.0.1:47000

# 3. Start the frontend (proxies /api to the backend).
cd frontend && bun install && bun run dev
```

Open the Vite URL it prints (usually http://localhost:5173).

DuckDB only, without the container:

```bash
cargo run -- --no-postgres
```

### Single-process production build

Build the frontend, then the server will serve it directly:

```bash
cd frontend && bun run build      # emits frontend/dist
cd .. && cargo run                # /api + static site on :47000
```

## Options

| Flag / env | Default | Meaning |
| --- | --- | --- |
| `--addr` / `LEDGER_ADDR` | `127.0.0.1:47000` | API listen address |
| `--postgres-url` / `DATABASE_URL` | `host=localhost port=5433 …` | Postgres connection string |
| `--no-postgres` | off | Start with DuckDB only |

## A note on `Analyze`

The **Analyze** toggle runs `EXPLAIN ANALYZE`, which **executes** the query to
collect real timing and cardinalities. Leave it off for planning-only
estimates; turn it on to compare estimates against reality (and to see which
engine is actually faster). Beware: with `ANALYZE`, a query that writes will
write.

## Future work

- Redshift / Snowflake engines (one `impl Engine` each) when an account is
  available — the plan-normalization layer is already engine-agnostic.
- Diff view: align equivalent operators across engines side by side.
- Persist and compare query-plan history.
