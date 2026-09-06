# Data View

Snapshot data explorer — Rust/Axum API serving supply, demand, and reference snapshots from Redshift, with a React frontend.

## CLI

The compiled **Rust binary is the single entrypoint** — serving, local
development, and deployment all live in one binary (`cargo run -- <command>`).
With no command, it serves (so the production `ENTRYPOINT ["./data_view"]` boots
the API directly).

```bash
cargo run -- <command>
```

| Command             | Description                                                                               |
| ------------------- | ----------------------------------------------------------------------------------------- |
| `serve`             | Run the API server on port 8050 (one-shot; the production entrypoint)                     |
| `start`             | Dev: frontend (vite) + backend together, with backend hot-reload by default; stops vite when the API exits |
| `start --install`   | Install frontend deps first, then start                                                   |
| `start --no-reload` | Start without backend hot-reload (one-shot `cargo run`)                                    |
| `install`           | Install frontend dependencies (`bun install`)                                             |
| `build`             | Build the frontend for production (`bun install --frozen-lockfile` + `bun run build`)     |
| `dev`               | Run the frontend dev server only (`bun run dev`)                                          |
| `deploy`            | Build and deploy the container image to Nimbus (via the `ouroboros` crate; needs `API_KEY`) |

In production the container runs the compiled binary directly (serving the API
on port 8050 and the pre-built `frontend/dist`).

## Development

```bash
# Frontend + backend together. The backend hot-reloads by default via bacon
# (one-time: cargo install bacon); pass --no-reload for a one-shot run.
cargo run -- start

# Offline: frontend + backend against a seeded, throwaway Postgres container
# (served over the Redshift-compatible wire). Needs Docker or Podman running.
cargo run -- start --test

# Just the API server (no vite), against the warehouse in your environment
# (DATA_WAREHOUSE_TYPE + REDSHIFT_*/SNOWFLAKE_*). On a missing DB or bad config
# it logs a clear error and exits cleanly — no panic.
cargo run -- serve
```

[bacon](https://github.com/Canop/bacon) is also configured for continuous check/lint/test feedback:

```bash
bacon            # cargo check on save
bacon clippy     # lint on save
bacon test       # tests on save
bacon serve      # run the API server, hot-reloading on change
```

## Logging

Controlled via `RUST_LOG` (defaults to `data_view=info,tower_http=info`):

```bash
RUST_LOG=data_view=debug cargo run        # SQL queries logged before execution
RUST_LOG=data_view=debug,tower_http=debug cargo run  # + HTTP request/response details
```

Failed queries are always logged at ERROR level with the full SQL and elapsed time.

## Build Features

The database backend is selected at compile time via feature flags:

```bash
cargo build                                            # Postgres/Redshift + Snowflake (default)
cargo build --no-default-features --features postgres  # Postgres/Redshift only
cargo build --no-default-features --features snowflake # Snowflake only
```

## API Endpoints

| Endpoint                                         | Description                       |
| ------------------------------------------------ | --------------------------------- |
| `GET /health`                                    | Health check                      |
| `GET /api/data_view/tables`                      | List available snapshot tables    |
| `GET /api/data_view/timestamps/{table}`          | Recent load timestamps (limit 50) |
| `GET /api/data_view/columns/{table}`             | Column metadata for a table       |
| `GET /api/data_view/column_values/{table}/{col}` | Distinct values for a column      |
| `GET /api/data_view/data/{table}`                | Query snapshot data with filters  |
