# Redhouse

Amazon Redshift analytics explorer — Rust/Axum API exposing deep performance and
cost insights for Amazon Redshift, with a React frontend.

Split out of `cocoon/warehouse` (which served both Redshift and Snowflake): this
app is Redshift-only. Its Snowflake counterpart is `cocoon/snowhouse`.

## CLI

The compiled **Rust binary is the single entrypoint** — serving, local
development, and deployment all live in one binary (`cargo run -- <command>`).
With no command, it serves (so the production `ENTRYPOINT ["./redhouse"]` boots
the API directly).

```bash
cargo run -- <command>
```

| Command              | Description                                                                          |
| -------------------- | ------------------------------------------------------------------------------------ |
| `serve`              | Run the API server on port 8050                                                      |
| `start`              | Start the backend, wait until it is serving on :8050, then start the frontend (vite) |
| `start --install`    | Install frontend deps first, then start                                              |
| `start --reload`     | Start with backend hot-reload (via `bacon`, restarts the server on change)           |
| `install`            | Install frontend dependencies (`bun install`)                                        |
| `build`              | Build the frontend for production (`bun install --frozen-lockfile` + `bun run build`)|
| `dev`                | Run the frontend dev server only (`bun run dev`)                                     |
| `deploy`             | Build and deploy the container image to Nimbus (via the `ouroboros` crate; needs `API_KEY`) |

In production the container runs the compiled binary directly (`redhouse serve`,
serving the API on port 8050 and the pre-built `frontend/dist`).

## Development

This app is **Redshift-only** — the connector is fixed at compile time, so a
live Redshift connection is required to boot (set the Redshift connection env
vars, see `.env.example`).

```bash
# Frontend + backend together, with backend hot-reload via bacon.
cargo run -- start --reload

# Or just the API server (no vite):
cargo run -- serve
```

[bacon](https://github.com/Canop/bacon) is also configured for continuous feedback:

```bash
bacon            # cargo check on save
bacon serve      # run the API server with hot reload (Redshift backend)
bacon clippy     # lint on save
bacon test       # tests on save
```

## Logging

Controlled via `RUST_LOG` (defaults to `redhouse=info,tower_http=info`):

```bash
RUST_LOG=redhouse=debug cargo run        # SQL queries logged before execution
RUST_LOG=redhouse=debug,tower_http=debug cargo run  # + HTTP request/response details
```

Failed queries are always logged at ERROR level with the full SQL and elapsed time.

## Build

```bash
cargo build   # Postgres/Redshift connector (the only backend)
```

## API Endpoints

| Endpoint                                         | Description                          |
| ------------------------------------------------ | ------------------------------------ |
| `GET /health/live`                               | Liveness check                       |
| `GET /api/warehouse-type`                        | Active warehouse type (always redshift) |
| `GET /api/warehouse/redshift/*`                  | Redshift analytics & performance     |

See `/swagger-ui` for the full endpoint catalog.
