# Cocoon

Cocoon holds dedicated applications (not library code) and the reusable crates
they share. It's a single Cargo workspace: one `target/`, one `Cargo.lock`,
versions and lints centralised in the root `Cargo.toml`.

```
cocoon/
├── apps/      deployable binaries
│   ├── data_view   snapshot data viewer  (Axum API + React frontend, port 8050)
│   ├── redhouse    Redshift analytics    (Axum API + React frontend, port 8050)
│   ├── snowhouse   Snowflake analytics   (Axum API + React frontend, port 8050)
│   ├── tako        data ingest API       (Axum, port 3000)
│   ├── nyx         LLM-powered dev CLI
│   └── warden      control layer: deploys the other apps into a tenant (CLI)
└── crates/    reusable libraries (database, service-kit, ouroboros, deploy-catalog, …)
```

`data_view`, `redhouse` and `snowhouse` are **Rust-only**: a single binary is the entrypoint
for serving, local development, _and_ deployment (it replaced the old Python
`typer` CLI). Deployment goes through the [`ouroboros`](crates/ouroboros) crate,
driven by the deploy specs in [`deploy-catalog`](crates/deploy-catalog).

## Control layer vs application layer

The apps split in two. The **application layer** (`data_view`, `redhouse`,
`snowhouse`, …) is tenant- and credential-specific: each service needs warehouse
credentials to read data. The **control layer** — [`warden`](apps/warden) — is
relaxed: its only credential is the tenant's Nimbus `API_KEY` (a deploy token), and
it deploys the other apps into whichever tenant that key selects. See
[`apps/warden`](apps/warden/README.md) for the credentials model (deploy *by
reference*).

## Prerequisites

- **Rust** toolchain (`rustup`)
- **bun** — frontend dev/build (`data_view`, `redhouse`, `snowhouse`)
- **cargo-watch** _(optional)_ — backend hot-reload for `start --reload`
  (`cargo install cargo-watch`)
- **`API_KEY`** env var — a Nimbus platform token, required for `deploy`

## Running the app CLIs

Each app exposes the same subcommands. `serve` is the default, so running the
binary with no command boots the API server (this is what the container's
`ENTRYPOINT` does).

| Command             | What it does                                                                |
| ------------------- | --------------------------------------------------------------------------- |
| `serve` _(default)_ | Run the API server (each on :8050 — run one at a time locally)               |
| `start`             | Run frontend (vite) + backend together; stops vite when the API exits       |
| `start --install`   | `bun install` first, then `start`                                           |
| `start --reload`    | `start` with backend hot-reload (needs `cargo-watch`)                       |
| `install`           | Install frontend deps (`bun install`)                                       |
| `build`             | Build the frontend for production (`bun install --frozen-lockfile` + build) |
| `dev`               | Run the frontend dev server only (`bun run dev`)                            |
| `deploy`            | Build and ship the container image to Nimbus (needs `API_KEY`)                |

### Option A — run from the workspace (no install)

The usual way during development. From `cocoon/`:

```bash
# Serve the API locally with the in-memory DuckDB backend (no external DB)
cargo run -p data_view --features duckdb
cargo run -p redhouse --features duckdb
cargo run -p snowhouse --features duckdb

# Any subcommand goes after `--`
cargo run -p data_view -- start --reload     # full local dev, hot-reload (uses duckdb)
cargo run -p redhouse -- dev                 # frontend only
cargo run -p data_view -- deploy             # build + deploy (needs API_KEY)
```

#### Local database backend

The backend is selected at **runtime** by `DATA_WAREHOUSE_TYPE` (default
`duckdb`), but each backend must also be compiled in via a **feature flag**.
The default build features are `postgres` + `snowflake`, so a bare
`cargo run -p data_view` panics with _"Unknown or disabled database backend:
'duckdb'"_. For local work, add the `duckdb` feature (in-memory, zero setup):

```bash
cargo run -p data_view --features duckdb                       # duckdb + the defaults
cargo run -p data_view --no-default-features --features in_memory  # leaner: duckdb + sqlite only
```

The `start`/`start --reload` commands already build the backend with `--features
duckdb` for you. To run against Postgres/Redshift or Snowflake instead, set
`DATA_WAREHOUSE_TYPE` and the matching connection env vars (see `.env.example`).

### Option B — install the CLI onto your PATH

Compiles a release binary into `~/.cargo/bin/` so you can call it by name:

```bash
cargo install --path apps/data_view     # installs the `data_view` binary
cargo install --path apps/redhouse      # installs the `redhouse` binary
cargo install --path apps/snowhouse     # installs the `snowhouse` binary
```

Then:

```bash
data_view                 # serve (default)
data_view start --reload  # local dev
data_view deploy          # deploy
redhouse serve
```

> The dev/build/deploy commands operate on the source checkout the binary was
> compiled from (the path is baked in at build time). Re-run `cargo install`
> after moving or changing the checkout, or just use Option A for day-to-day work.

### Build a release binary without installing

```bash
cargo build --release -p data_view     # → target/release/data_view
```

## Deploying

```bash
export API_KEY="<nimbus-platform-token>"
cargo run -p data_view -- deploy       # or: data_view deploy
```

`deploy` zips the app directory (honouring `.dockerignore`), uploads it as an
image artifact, waits for the platform build to finish, then creates or updates
the webapp — all via the `ouroboros` crate. The image/service name, build args,
secrets and Dockerfile are configured per app in `apps/<app>/src/cli.rs`.

## Other apps

- **tako** — `cargo run -p tako` starts the ingest API on :3000. See
  [apps/tako/README.md](apps/tako/README.md).
- **nyx** — an LLM-powered developer CLI; install and usage in
  [apps/nyx/README.md](apps/nyx/README.md).

## Development

```bash
cargo build --workspace        # build everything
cargo test  --workspace        # test everything
cargo clippy --workspace --all-targets
bacon                          # continuous check/lint/test (per app)
```

## Testing

Where it can, the suite stays **in-process** (no Docker, no network): `tako`, for
example, has an end-to-end test that drives its real Axum router with
`tower::ServiceExt::oneshot` against a file-backed, in-process **DuckDB** —
covering create-table → upload → `COPY` → inspect without a live warehouse. It
lives in tako's `testing` module, so it runs under
`cargo test -p tako --features testing` (a plain `cargo test --workspace` does
not compile that module — CI must pass the feature).
`data_view` goes further with testcontainers (`tests/api_it.rs`): a real Postgres
over the wire plus a Snowflake mock — but those need a Docker/Podman socket, so
they are skipped (they fail loudly) on a machine without one.

> ⚠️ **Fidelity gap: we never test against a real Redshift or Snowflake.** This is
> a known crack in our test fidelity. The production warehouses are only ever
> _approximated_: `tako` tests on DuckDB, and `data_view`'s integration tests use
> Postgres + a Snowflake mock. Redshift in particular is a fork of PostgreSQL
> 8.0.2 — modern PG functions are absent and several behaviours differ — so
> Postgres is **not** a faithful stand-in. Dialect-specific paths (Redshift's
> `COPY … IAM_ROLE`, Snowflake's `COPY INTO … STORAGE_INTEGRATION`,
> `MATCH_BY_COLUMN_NAME`) and real warehouse concurrency are exercised for the
> first time **in production**. Treat green tests as "the plumbing is correct",
> not "this works on the live warehouse".

> **TODO — look into testcontainers.** DuckDB exercises the happy path cheaply,
> but it is not Redshift or Snowflake: SQL-dialect quirks (e.g. Redshift's
> `COPY … IAM_ROLE`, Snowflake's `COPY INTO … STORAGE_INTEGRATION`) and real
> concurrency only show up against the actual engines. We should evaluate
> [`testcontainers`](https://docs.rs/testcontainers) to run disposable,
> seeded Postgres/Redshift (and an S3 mock) for the integration tier, so the
> dialect-specific code paths are covered against something closer to production.
> The DuckDB in-process tests stay as the fast default.
