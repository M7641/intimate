# dv-database

Database abstraction layer with support for multiple backends via a unified `Database` trait.

## Architecture

Connectors are split into two families:

- **In-memory** — embedded databases that need no external service (DuckDB, SQLite)
- **External** — remote databases via native protocols (Postgres/Redshift) or REST API (Snowflake)

## Features

There are **no default features** — each consumer enables exactly the backends
it needs (the workspace dependency is `default-features = false`). At least one
backend must be enabled or the build fails with a clear message.

| Feature          | Description                                          |
| ---------------- | ---------------------------------------------------- |
| `duckdb`         | Embedded DuckDB                                      |
| `sqlite`         | Embedded SQLite                                      |
| `postgres`       | Native Postgres wire protocol (Redshift)             |
| `postgres-async` | Async Postgres/Redshift (tokio-postgres + deadpool)  |
| `snowflake`      | Snowflake REST API v2 (includes Nimbus OAuth)          |

## Examples

### DuckDB (in-memory)

```bash
cargo run --example duckdb --features duckdb
```

### DuckDB merge (upsert)

```bash
cargo run --example merge --features duckdb
```

### SQLite (in-memory)

```bash
cargo run --example sqlite --features sqlite
```

### Redshift

```bash
cargo run --example postgres --features postgres
```

### Snowflake

```bash
cargo run --example snowflake --features snowflake
```
