# Structured Logging

> Status: **Implemented** — `src/main.rs`, throughout `src/routes/`

## What

Every log entry is emitted as a set of machine-parseable key-value pairs rather than a free-form string. In production the output is JSON; in development it is pretty-printed for human readability.

## Why

| Without structured logging                   | With structured logging                                                                              |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `"Error processing request for table users"` | `{"level":"ERROR","error.kind":"database","table":"users","request_id":"abc-123","elapsed_ms":1204}` |
| Requires regex to parse                      | Log aggregators (Datadog, Loki, CloudWatch) index fields automatically                               |
| Cannot filter by field                       | `jq`, Kibana, or Grafana queries can filter by any field                                             |
| Cannot correlate across lines                | `request_id` ties all lines from one request together                                                |
| Grep is the only tool                        | Dashboards, alerts, and anomaly detection become possible                                            |

A log line that cannot be filtered, correlated, or parsed by a machine is noise. In an incident, noise costs minutes — and minutes cost money.

## How — This Repo

### Environment-aware formatting (`src/main.rs`)

```rust
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("data_view=info,tower_http=info"));

    let is_production = std::env::var("RUST_ENV")
        .map(|v| v == "production")
        .unwrap_or(false);

    if is_production {
        tracing_subscriber::fmt().with_env_filter(env_filter).json().init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).pretty().init();
    }
}
```

Two formats exist because developers need readable output locally, while production pipelines need structured JSON.

### Handler instrumentation (`src/routes/`)

```rust
#[tracing::instrument(skip(state), fields(table = %table_name))]
pub async fn columns_handler(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
) -> Result<Json<Vec<ColumnInfoResp>>, AppError> { ... }
```

The `#[tracing::instrument]` macro automatically creates a span with the function name and any `fields(...)` you declare. Every log line inside inherits these fields.

### Log levels

| Level    | When                                          | Example                         |
| -------- | --------------------------------------------- | ------------------------------- |
| `error!` | Something is broken and needs human attention | DB connection failure           |
| `warn!`  | Unexpected but handled                        | Slow query, validation failure  |
| `info!`  | Normal operational milestones                 | Server started, query completed |
| `debug!` | Detailed troubleshooting context              | SQL text, raw parameters        |
| `trace!` | Very fine-grained (rarely in prod)            | Byte-level protocol data        |

**Rule of thumb**: run `info` in production. Temporarily increase a single module when investigating:

```bash
RUST_LOG="data_view::state=debug,tower_http=info" ./dv start
```
