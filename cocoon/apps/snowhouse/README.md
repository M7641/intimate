# Snowhouse

Snowflake analytics explorer — Rust/Axum API exposing deep performance and cost
insights for Snowflake, with a React frontend.

Split out of `cocoon/warehouse` (which served both Redshift and Snowflake): this
app is Snowflake-only. Its Redshift counterpart is `cocoon/redhouse`.

## What it does

Snowhouse turns Snowflake's raw query history into focused, actionable views.
The idea has two sides:

1. **Collect.** Snowhouse pulls the record of every query the connecting role can
   see — execution time, bytes scanned, warehouse and size, queue time, credits,
   and the SQL text itself — into a table it owns. This side-steps Snowflake's
   10k-row-per-call history limit (see [Query history cache](#query-history-cache))
   and lets history accumulate well beyond the 7-day `INFORMATION_SCHEMA` window.

2. **Show.** The React frontend reads that table back through a set of analytics
   endpoints, each answering one question: which queries cost the most, how each
   warehouse is utilised and whether it is right-sized, where queries are queueing
   for compute, what a query's execution plan and per-operator cost look like,
   spend by day / tag / query type, storage by table, and repeated or failed
   queries.

**Collection runs automatically.** By default `serve` starts an in-process
background job that refreshes the cache **every hour**, so the views stay current
with no external scheduler. It is on out of the box; set
`SNOWHOUSE_CACHE_AUTO_REFRESH=false` to turn it off (for example when a cron job
runs `cache refresh` instead), or `SNOWHOUSE_CACHE_REFRESH_SECS` to change the
interval. The very first run still needs a one-time `cache init` + `backfill` to
create and seed the table.

## CLI

The compiled **Rust binary is the single entrypoint** — serving, local
development, and deployment all live in one binary (`cargo run -- <command>`).
With no command, it serves (so the production `ENTRYPOINT ["./snowhouse"]` boots
the API directly).

```bash
cargo run -- <command>
```

| Command           | Description                                                                               |
| ----------------- | ----------------------------------------------------------------------------------------- |
| `serve`           | Run the API server on port 8050                                                           |
| `start`           | Start the backend, wait until it is serving on :8050, then start the frontend (vite)      |
| `start --install` | Install frontend deps first, then start                                                   |
| `start --reload`  | Start with backend hot-reload (via `bacon`, restarts the server on change)                |
| `install`         | Install frontend dependencies (`bun install`)                                             |
| `build`           | Build the frontend for production (`bun install --frozen-lockfile` + `bun run build`)     |
| `dev`             | Run the frontend dev server only (`bun run dev`)                                          |
| `deploy`          | Build and deploy the container image to Nimbus (via the `ouroboros` crate; needs `API_KEY`) |
| `cache init`      | Create the query-history cache table (see below)                                          |
| `cache backfill`  | Populate the cache with up to 7 days of history (`--days N`, 1–7)                          |
| `cache refresh`   | Top up the cache with queries since the last cached one (hourly job)                       |
| `cache status`    | Print cache stats: row count, covered time span, last ingest                              |

In production the container runs the compiled binary directly (`snowhouse serve`,
serving the API on port 8050 and the pre-built `frontend/dist`).

## Development

This app is **Snowflake-only** — the connector is fixed at compile time, so a
live Snowflake connection is required to boot (set the `SNOWFLAKE_*` env vars,
see `.env.example`).

```bash
# Frontend + backend together, with backend hot-reload via bacon.
cargo run -- start --reload

# Or just the API server (no vite):
cargo run -- serve
```

[bacon](https://github.com/Canop/bacon) is also configured for continuous feedback:

```bash
bacon            # cargo check on save
bacon serve      # run the API server with hot reload (Snowflake backend)
bacon clippy     # lint on save
bacon test       # tests on save
```

## Warehouse selection

snowhouse only reads metadata and query history — it never runs compute-heavy
work — so it **always runs on the smallest warehouse** the role can see. This is
automatic and needs no configuration: at startup the connector runs
`SHOW WAREHOUSES` (free — it executes on the services layer), picks the one with
the cheapest credit rate, and runs every statement there. The pick is resolved
once per process and logged.

This deliberately overrides both `SNOWFLAKE_WAREHOUSE` and the warehouse from the
Nimbus connection — snowhouse should never burn credits on an oversized warehouse.
(Mechanically, `snowhouse` calls `database::enable_smallest_warehouse()` at
startup; the shared connector leaves the behaviour off for every other app.)

Note that `SHOW WAREHOUSES` reflects *visibility*, not the `USAGE` privilege — if
the smallest visible warehouse isn't usable by the role, the first query fails
with a clear error rather than silently falling back.

## Query history cache

`INFORMATION_SCHEMA.QUERY_HISTORY` returns at most **10,000 rows per call** and
has no `OFFSET`, so a busy week cannot be read in one shot without account-admin
access to `ACCOUNT_USAGE`. Snowhouse works around this by owning a persistent
table it fills itself, then queries freely.

> **The cache is the only history source.** Every analytics endpoint reads
> query history exclusively from this table — there is no `information_schema`
> fallback — so `serve` fails at startup if the table does not yet exist. Run
> `cache init` (and `backfill`) before the first `serve`.

The table name is fixed at `SANDPIT.SNOWHOUSE_QUERY_HISTORY_CACHE` — not
configurable, so it is a stable, predictable target; the `SNOWHOUSE_` prefix
marks it as this app's table in the shared `sandpit` schema. The connecting role
just needs `CREATE TABLE` on that schema.

Then, once:

```bash
cargo run -- cache init        # create the table (DDL owned in src/cache/sql.rs)
cargo run -- cache backfill     # sweep the last 7 days into it
```

And on a schedule (hourly) to keep it current:

```bash
cargo run -- cache refresh      # add queries since the last cached one
```

How it stays correct:

- **No silent truncation.** History is paged by *time*, never by offset. Each
  window's `COUNT(*)` is probed; a window that saturates the 10k cap is bisected
  in time until every slice fits — so nothing is dropped.
- **Idempotent.** Rows are folded in with `MERGE … WHEN NOT MATCHED` on
  `query_id`, so overlapping windows, re-runs, and the refresh overlap window
  can never create duplicates. `backfill` and `refresh` share this one upsert.
- **Watermark.** `refresh` reads `MAX(end_time)` and re-scans from one hour
  before it, catching queries whose completion straddled the previous run.

Scheduling options (pick one):

- **In-process ticker (default)**: `serve` refreshes itself every
  `SNOWHOUSE_CACHE_REFRESH_SECS` (default 3600 — hourly). On out of the box; set
  `SNOWHOUSE_CACHE_AUTO_REFRESH=false` to disable. Simplest for a single instance.
  With several replicas each ticker writes independently — still correct thanks to
  the `MERGE`, just wasteful, so prefer the external job there.
- **External job** (recommended for multiple replicas): disable the ticker and run
  `cache refresh` from cron / a Nimbus workflow instead. Single writer, retriable,
  observable.

Scope: the cache holds exactly the queries the connecting **role** can see in
`QUERY_HISTORY` — no more. With admin / `IMPORTED PRIVILEGES` on `ACCOUNT_USAGE`,
this whole mechanism collapses to a plain `SELECT`.

## Logging

Controlled via `RUST_LOG` (defaults to `snowhouse=info,tower_http=info`):

```bash
RUST_LOG=snowhouse=debug cargo run        # SQL queries logged before execution
RUST_LOG=snowhouse=debug,tower_http=debug cargo run  # + HTTP request/response details
```

Failed queries are always logged at ERROR level with the full SQL and elapsed time.

## Build

```bash
cargo build   # Snowflake connector (the only backend)
```

## API Endpoints

| Endpoint                         | Description                              |
| -------------------------------- | ---------------------------------------- |
| `GET /health/live`               | Liveness check                           |
| `GET /api/warehouse-type`        | Active warehouse type (always snowflake) |
| `GET /api/warehouse/snowflake/*` | Snowflake analytics & cost insights      |

See `/swagger-ui` for the full endpoint catalog.
