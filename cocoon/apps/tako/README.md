# tako

Batch data ingest API for data warehouses. Accepts CSV, JSON, Parquet, and Avro files, converts to Parquet, uploads to S3, and loads into the configured warehouse (DuckDB, Redshift, or Snowflake).

There is not yet a route to handle real time ingestion. Nor does it use any database that would enable such a thing properly either.

## Supported file types

The accepted formats are defined by the `FileType` enum in the `parse` module (the single source of truth). The file's extension selects the reader; every format is normalised to Parquet before it reaches the warehouse.

| Format        | Extensions                 | Notes                                                                                                 |
| ------------- | -------------------------- | ----------------------------------------------------------------------------------------------------- |
| CSV           | `.csv`                     |                                                                                                       |
| Parquet       | `.parquet`                 |                                                                                                       |
| JSON / NDJSON | `.json` `.ndjson` `.jsonl` | Array vs newline-delimited is auto-detected.                                                          |
| Avro          | `.avro`                    |                                                                                                       |
| Vortex        | `.vortex` `.vx`            | Modern columnar format ([vortex.dev](https://vortex.dev)). Decoded via arrow-rs → Arrow IPC → Polars. |

All formats are built in (Vortex is no longer behind a feature flag).

### Candidate formats to add next

Each would slot into the `parse` module as a new `FileType` variant + reader:

- **Arrow IPC / Feather** (`.arrow`, `.feather`, `.ipc`) — Polars reads it natively (`IpcReader`); near-zero conversion cost since the warehouse path is already Arrow-shaped. Lowest-effort, highest-value addition.
- **ORC** (`.orc`) — common in the Hadoop/Hive world; pairs naturally with the Parquet/Avro lineage.
- **Excel** (`.xlsx`) — the format business users actually send; Polars supports it via the `calamine` reader behind a feature.
- **Compressed text** (`.csv.gz`, `.json.zst`) — transparently decompress (gzip/zstd) then dispatch on the inner extension; big wins on upload bandwidth.
- **Delta Lake / Iceberg snapshots** — not single files, but a natural extension if ingest ever needs to pull an existing table rather than a flat file.

## Quick Start

`tako` is a single binary with subcommands; `serve` is the default. The test
tooling is gated behind the `testing` feature (off by default), so this builds a
server-only binary:

```bash
cargo build
cargo run            # = `cargo run -- serve`, starts the server on 0.0.0.0:3000
```

To build/run the test tooling, add `--features testing` (see [CLI](#cli--test-tooling)).

## API Endpoints

| Method | Path                     | Description                                                                                                |
| ------ | ------------------------ | ---------------------------------------------------------------------------------------------------------- |
| POST   | `/table`                 | Create the staging table for a schema (`?table=<name>&schema=<name>`). Idempotent.                         |
| GET    | `/table`                 | List the tables in the staging schema (names only).                                                        |
| GET    | `/table/{name}`          | Column breakdown of one table (`404` if it doesn't exist).                                                 |
| POST   | `/upload`                | Upload a file (multipart, `?table=<name>` **required**, `&schema=<name>` defaults to `stage`). Max 100 MB. |
| GET    | `/health`                | Liveness — process is up (`{"status":"ok"}`). Always 200.                                                  |
| GET    | `/health/ready`          | Readiness — pings the warehouse pool; `200` ready, `503` if the backend is unreachable.                    |
| GET    | `/instance_health`       | CPU usage, memory stats, uptime                                                                            |
| GET    | `/metrics`               | Prometheus metrics (request rate / errors / duration)                                                      |
| GET    | `/swagger-ui`            | Interactive OpenAPI docs                                                                                   |
| GET    | `/api-docs/openapi.json` | Machine-readable OpenAPI 3 spec                                                                            |

The OpenAPI spec is generated from the handlers (`utoipa`) and mounted via the
shared `service_kit::openapi::swagger_ui` helper, so every cocoon service exposes
its spec and UI at the same paths.

Table creation (DDL) and data ingestion are **separate concerns**: `/upload`
only `COPY`s into an existing table and does not create it. Create the table
first with `/table`; uploading to a missing table fails. This keeps each request
a single warehouse statement (no multi-statement transaction to coordinate).

### Example: create the table, then upload

```bash
# 1. Create stage.sample_data from the `sample_data` schema (idempotent)
curl -X POST "http://localhost:3000/table?table=sample_data&schema=sample_data"

# 2. Load a file into it (table is required; schema defaults to `stage`)
curl -X POST "http://localhost:3000/upload?table=sample_data&schema=sample_data" \
  -F "file=@data.csv"
```

### Example: inspect tables

```bash
# List the staging tables (top-level overview)
curl http://localhost:3000/table
# → {"schema":"stage","tables":["sample_data", ...]}

# Column breakdown of one table (404 if it doesn't exist)
curl http://localhost:3000/table/sample_data
# → {"schema":"stage","table":"sample_data",
#    "columns":[{"name":"id","data_type":"INTEGER","nullable":false,"position":1}, ...]}
```

> **Local DuckDB note:** the default backend is in-memory DuckDB (`:memory:`),
> and the connection pool opens _independent_ in-memory databases — so a table
> created via `/table` on one pooled connection is invisible to the `/upload`
> `COPY` on another. For the two-step flow to share state locally, set
> `DUCKDB_PATH` to a file (a persistent DuckDB). The warehouse backends
> (Redshift / Snowflake) persist server-side and are unaffected.

## Data Vault enrichment

A schema **opts in** to Data Vault (raw-vault) enrichment simply by declaring a
`business_key` in its `keys` block. When it does, every uploaded row is enriched
— **insert-only** — with standard DV metadata before the `COPY`:

| Column          | Meaning                                                                               |
| --------------- | ------------------------------------------------------------------------------------- |
| `<entity>_hk`   | SHA-256 of the business key column(s) — the hub hash key.                             |
| `hashdiff`      | SHA-256 of all business columns — a per-row content fingerprint for change detection. |
| `load_date`     | When the batch was loaded (UTC).                                                      |
| `record_source` | Provenance, `tako/<schema>/<table>`.                                                  |
| `load_id`       | A UUID shared by every row of one upload, tying the batch together.                   |

`/table` adds these columns to the DDL and `/upload` populates them; the column
set and order are the single source of truth (`schema::enriched_columns`), so the
Parquet lines up with the table for both position-matched (`DuckDB`, Redshift) and
name-matched (Snowflake) `COPY`.

**This is how re-uploads are handled** (the "idempotency" question): tako does not
reject a repeated file. DV is append-only, so a re-load is _recorded_ — the rows
carry the same `<entity>_hk` and `hashdiff` but a fresh `load_id`/`load_date`, so
they are fully traceable and deduplicable downstream (by `hashdiff` per key)
rather than a silent, untracked duplicate.

A schema with **no** `business_key` is loaded unchanged (no enrichment).

## FAQ

### How do I load a new column into an existing table?

Options below are written up so we can pick deliberately.
The decisive constraint is how the `COPY` matches columns:

- DuckDB and Redshift `COPY` are **position-matched** (column N of the Parquet →
  column N of the table). Widening a table and then loading files of different
  widths silently misaligns unless every file carries every column in the same
  order.
- Snowflake `COPY INTO … MATCH_BY_COLUMN_NAME` is **name-matched**, so it
  tolerates missing/extra columns per file.

Also note `/table` only does `CREATE TABLE IF NOT EXISTS` — it never alters an
existing table — so any of these needs a deliberate mechanism, not just an
edited schema file.

**Option A — add a satellite (Data Vault native).** Leave the existing table
alone; put the new attribute(s) in a _new_ table keyed by the same hub hash key
(`<entity>_hk`) plus its own `load_date`/`hashdiff`. New data lands in the
satellite; downstream joins on the hash key to get the wide view.

- _Pros:_ append-only and immutable — no DDL on live tables, full history kept,
  old queries unaffected, parallel-load friendly. It is exactly the model the DV
  enrichment already sets up (the hash key is the join seam).
- _Cons:_ more tables and a join to reassemble a row. tako would need to express
  "this column group is a satellite of that hub" — most cheaply by treating each
  satellite as its own schema + `stage.<table>_sat_<name>` table that re-derives
  the same `<entity>_hk` from the business key, loaded via its own `/table` +
  `/upload`. No new endpoint, but a modelling convention to define.

**Option B — evolve the table (additive `ALTER TABLE`).** Bump the schema
version, and run `ALTER TABLE … ADD COLUMN` so the one wide table gains the
column (old rows get `NULL`); new uploads include it.

- _Pros:_ one wide table, no joins — the mental model most people expect.
- _Cons:_ mutates a live table, and it forces a loading change: position-matched
  `COPY` breaks the moment old and new files differ in width, so this realistically
  requires **switching DuckDB/Redshift loads to name-matched** too (or strictly
  appending columns at the end _and_ always sending every column). It also needs
  a real migration step (a new `/table` verb or a `migrate` path) and
  drift/versioning tracking — giving up some of today's "each request is one
  statement" simplicity.

**Option C — versioned table (`stage.<table>_v2`).** Treat a new column set as a
new schema version (the registry already supports `{key}_v{N}.json`) loaded into
a new table; a view unions/coalesces v1 + v2.

- _Pros:_ tables stay immutable, clean cutover, no `ALTER`.
- _Cons:_ table/version proliferation and union plumbing downstream.

**Option D — semi-structured overflow column.** Keep a catch-all `VARIANT`
(Snowflake) / `SUPER` (Redshift) / `JSON` (DuckDB) column for unmodelled fields;
new attributes land there until promoted to real columns.

- _Pros:_ zero DDL for new fields, maximally flexible.
- _Cons:_ weak typing, awkward querying, uneven across backends, and it bypasses
  the schema validation that is otherwise a core feature.

**Leaning:** A is the most consistent with tako's append-only, DV-shaped design
and needs no live-table DDL; B is the most familiar but pulls the loader toward
name-matched everywhere and a migration mechanism. The choice is really "stay
append-only and join later (A/C)" vs "mutate in place and keep it wide (B/D)".

## Security & deployment

**`tako` performs no authentication or authorization, by design.** Every endpoint
— including `/upload`, which writes to the warehouse — is open to any caller that
can reach the process. This is deliberate: auth is a cross-cutting platform
concern, so `tako` is meant to run **only behind a gateway / reverse proxy that
authenticates and authorizes requests** before they reach it, on a network that
is not directly reachable by untrusted clients.

Consequences to respect when deploying:

- **Never expose `tako` directly to an untrusted network.** Without the gateway
  in front, anyone who can open a socket can ingest arbitrary data and create
  tables. Treat reachability as equivalent to write access to the warehouse.
- The gateway owns authentication, authorization, and **per-client** rate
  limiting. `tako` does not duplicate those, but it does apply a coarse
  **global** throughput backstop on the load-bearing routes (`/upload`,
  `/table*`) — `RATE_LIMIT_RPS`, default 100 req/s, returning `429` — so a flood
  cannot overwhelm the warehouse even if the gateway misbehaves. Probes
  (`/health*`, `/metrics`) are exempt.
- Identifier validation (table/schema names) _is_ done in-process — that is data
  integrity / injection safety, not access control, and is independent of the
  gateway.

If you ever need `tako` to stand alone, add an auth layer (the shared
`service-kit` middleware is the natural place) before exposing it.

## Background processing

Uploads are processed **synchronously** inside the request handler: parse →
schema validation → Parquet → S3 upload → warehouse `COPY`, all before the
response returns. There is no job queue and no background worker.

An earlier design queued each upload and drained it from a background task (a
`scheduler` module with a `WorkerHandle`); it was removed once processing became
synchronous. If ingest ever needs to return immediately and process later (large
files, retries, rate-limited warehouse loads), reintroduce that pattern:

1. Hold an `mpsc::Sender<Job>` in `AppState`; the handler sends a `Job` and
   returns `202 Accepted` instead of doing the work inline.
2. On startup, `tokio::spawn` a worker that loops over the `mpsc::Receiver`,
   doing the parse → S3 → `COPY` steps that currently live in the handler.
3. For clean shutdown, select the worker loop against a `watch`/`CancellationToken`
   signal, and `await` the worker's `JoinHandle` after `axum::serve` returns so
   in-flight jobs finish.

See the Tokio guide on graceful shutdown for the channel + signal + join wiring:
<https://tokio.rs/tokio/topics/shutdown>.

## CLI — test tooling

These subcommands live behind the **`testing` Cargo feature, which is off by
default** — a plain `cargo build`/`cargo run` produces a server-only binary
without this code or its dependencies (see [Layout](#testing--tooling-not-production)).
You must pass `--features testing` to compile and run them:

```bash
# Generate sample files (CSV, Parquet, JSON, Avro)
cargo run --features testing -- gen-data [--output-dir <path>] [--rows <n>]

# Upload sample files to a running server
cargo run --features testing -- test-upload [--base-url <url>] [--schema <name>]

# HTTP load benchmark of the /upload endpoint
cargo run --release --features testing -- benchmark [--base-url <url>] [--schema <name>] \
  [--concurrency <n>] [--requests <n>] [--format <fmt>] [--rows <n>] [--warmup <n>]

# Micro-benchmark the in-memory parse_data hot path (Criterion)
cargo run --release --features testing -- bench-parse
```

Without `--features testing`, `tako --help` lists only `serve`; the subcommands
above are not compiled in.

| Flag            | Default                       | Notes                                      |
| --------------- | ----------------------------- | ------------------------------------------ |
| `--base-url`    | `http://localhost:3000`       | API base URL                               |
| `--schema`      | `sample_data`                 | Schema name for upload                     |
| `--concurrency` | `10`                          | Max concurrent requests (benchmark only)   |
| `--requests`    | `50`                          | Total requests to send (benchmark only)    |
| `--format`      | `parquet`                     | `csv`, `parquet`, `json`, `avro`, or `all` |
| `--rows`        | `100` (gen) / `20000` (bench) | Rows per generated file                    |
| `--warmup`      | `5`                           | Warmup requests excluded from stats        |

## Database Examples (database)

```bash
# DuckDB in-memory
cargo run -p database --example duckdb

# DuckDB merge (upsert)
cargo run -p database --example merge

# PostgreSQL / Redshift (requires REDSHIFT_* env vars)
cargo run -p database --example postgres --features postgres

# Snowflake via Nimbus OAuth (requires API_KEY)
cargo run -p database --example snowflake --features snowflake
```

## Environment Variables

| Variable                                                                                        | Description                                                                                                                                     | Default                             |
| ----------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------- |
| `RUST_LOG`                                                                                      | Log level filter                                                                                                                                | `info,tako=debug`                   |
| `SCHEMA_DIR`                                                                                    | Directory holding the schema definitions                                                                                                        | dev fallback: `<workspace>/schemas` |
| `LOCAL_STORAGE_ROOT`                                                                            | Local blob-storage root (DuckDB backend only)                                                                                                   | dev fallback: `<workspace>/mock_s3` |
| `DATA_LAKE`                                                                                     | S3 data-lake bucket. **Required** for the postgres / redshift / snowflake backends — startup fails if unset (no default).                       | — (required)                        |
| `S3_BUCKET`                                                                                     | Object-key prefix within the data lake                                                                                                          | `$TENANT`                           |
| `MAX_CONCURRENT_UPLOADS`                                                                        | Uploads processed at once; bounds peak memory (≈ this × a few × max file size) and concurrent COPYs. Excess requests are shed with `503`.       | `4`                                 |
| `UPLOAD_TIMEOUT_SECS`                                                                           | Per-request timeout for `/upload` (returns `408`)                                                                                               | `120`                               |
| `DB_MAX_CONNECTIONS`                                                                            | Warehouse connection-pool size (`ApiDbActions`). Each in-flight upload holds one for its COPY.                                                  | `4`                                 |
| `DB_STATEMENT_TIMEOUT_SECS`                                                                     | Server-side warehouse statement timeout (Redshift/Snowflake); aborts a runaway COPY and frees the connection. `0` disables.                     | `120`                               |
| `RATE_LIMIT_RPS`                                                                                | Global request-rate cap on `/upload` + `/table*` (returns `429`); probes exempt. Defensive backstop, not per-client fairness.                   | `100`                               |
| `GLOBAL_TIMEOUT_SECS`                                                                           | Per-request timeout for the non-upload routes (returns `504`)                                                                                   | `30`                                |
| `REDSHIFT_HOST`, `REDSHIFT_PORT`, `REDSHIFT_DATABASE`, `REDSHIFT_USERNAME`, `REDSHIFT_PASSWORD` | Redshift/Postgres connection                                                                                                                    | —                                   |
| `REDSHIFT_IAM_ROLE`                                                                             | IAM role the Redshift `COPY` uses to read S3                                                                                                    | `tenant`                            |
| `API_KEY`                                                                                       | Nimbus OAuth key for Snowflake backend                                                                                                            | —                                   |
| `SNOWFLAKE_STORAGE_INTEGRATION`                                                                 | Snowflake storage integration the `COPY INTO` uses to read S3. **Required** for the snowflake backend — the upload fails if unset (no default). | — (required)                        |

> `SCHEMA_DIR` / `LOCAL_STORAGE_ROOT` are resolved at runtime. When unset, they
> fall back to a path relative to this crate's source tree (`CARGO_MANIFEST_DIR`,
> a compile-time constant) — convenient for `cargo run` in development, but that
> path does not exist in a container, so **deployments must set `SCHEMA_DIR`**
> (and `LOCAL_STORAGE_ROOT` if using the local/DuckDB backend).

## Development

```bash
bacon              # default job: check (watches for changes)
bacon tako         # run the server with hot-reload on change
cargo test         # run all tests
```

## Layout

A library plus a thin binary. All the logic lives in `lib.rs` (so it uses
`crate::` paths and is reachable by tests/benches); `main.rs` just awaits
`tako::run()`. The generic pieces are separate workspace libraries.

### This crate (`tako`)

| File       | Purpose                                                    |
| ---------- | ---------------------------------------------------------- |
| `main.rs`  | Thin entry point — `#[tokio::main]` awaiting `tako::run()` |
| `lib.rs`   | CLI + dispatch (`run`), `create_router`, the `serve` loop  |
| `routes/`  | HTTP handlers (`upload`, `table`, `health`)                |
| `state.rs` | `AppState` (schema registry, blob client, config)          |
| `parse.rs` | File parsing (CSV, JSON, Parquet, Avro, Vortex)            |
| `error.rs` | API error type → HTTP response                             |
| `testing/` | Test/bench tooling, never on the `serve` path (see below)  |

### `testing/` — tooling, not production

Grouped apart from the server code and gated behind the `testing` Cargo feature
(off by default). A production `cargo build --release` compiles none of it — and
none of its dependencies (`criterion`, `reqwest`, `rand`). Only the
`gen-data` / `test-upload` / `benchmark` / `bench-parse` subcommands use it, and
only when built with `--features testing`:

| File                  | Purpose                                                    |
| --------------------- | ---------------------------------------------------------- |
| `testing/testkit.rs`  | Sample-data generation, upload smoke test, HTTP load bench |
| `testing/bench.rs`    | `parse_data` Criterion micro-benchmark                     |
| `testing/examples.md` | Worked examples for the test-tooling subcommands           |

### Workspace libraries it depends on

| Crate         | Purpose                                               |
| ------------- | ----------------------------------------------------- |
| `service-kit` | Shared axum scaffolding (graceful shutdown, …)        |
| `schema`      | Schema registry and validation                        |
| `database`    | Database trait + DuckDB, Postgres, Snowflake backends |
| `blobs`       | S3 / object-storage client                            |
