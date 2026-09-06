# Metrics & the RED Method

## What

Numerical time-series data exposed at `GET /metrics` in Prometheus exposition format. A Prometheus server scrapes this endpoint periodically and stores the data for querying, alerting, and dashboards.

## Why

Logs tell you **what happened**. Metrics tell you **how much, how fast, and how often**.

| Question                                | Log-based answer                                       | Metric-based answer                                                      |
| --------------------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------ |
| "How many requests/sec are we serving?" | `grep -c` across all log files, recalculate every time | `rate(http_requests_total[5m])` — instant, pre-aggregated                |
| "What's our P95 latency?"               | Parse every log line, sort, compute percentile         | `histogram_quantile(0.95, ...)` — native Prometheus function             |
| "Is our error rate increasing?"         | Count error lines per minute, build a script           | Grafana auto-generates a chart from `http_requests_total{status=~"5.."}` |
| "Alert me when error rate > 5%"         | Write a cron job to tail logs                          | One Prometheus alerting rule, evaluated automatically                    |

Metrics are **constant cost** — incrementing a counter takes nanoseconds whether you serve 10 or 10 million requests. Logs scale linearly with traffic; metrics do not.

## The RED Method

Coined by Google SRE, RED stands for **Rate, Errors, Duration** — the three signals that cover the health of any request-serving service:

| Signal       | Type      | What it tells you                                |
| ------------ | --------- | ------------------------------------------------ |
| **R**ate     | Counter   | Throughput — how many requests per second        |
| **E**rrors   | Counter   | Reliability — how many requests failed           |
| **D**uration | Histogram | Latency — how long requests take (P50, P95, P99) |

If you only have three metrics, make them RED. They answer the three questions every on-call engineer asks: "Is it up? Is it fast? Is it correct?"

## How — This Repo

### HTTP RED metrics (`src/middleware.rs`)

```rust
pub async fn metrics_layer(
    matched_path: Option<MatchedPath>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().to_string();
    let path = matched_path
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string());

    let start = Instant::now();
    let response = next.run(req).await;
    let status = response.status().as_u16().to_string();

    metrics::counter!("http_requests_total",
        "method" => method.clone(), "path" => path.clone(), "status" => status
    ).increment(1);

    metrics::histogram!("http_request_duration_seconds",
        "method" => method, "path" => path
    ).record(start.elapsed().as_secs_f64());

    response
}
```

### Database metrics (`src/state.rs`)

```rust
metrics::counter!("db_queries_total", "status" => "success").increment(1);
metrics::histogram!("db_query_duration_seconds").record(elapsed.as_secs_f64());
metrics::counter!("db_slow_queries_total").increment(1);
metrics::gauge!("db_pool_connections_total").set(f64::from(ps.connections));
metrics::gauge!("db_pool_connections_idle").set(f64::from(ps.idle_connections));
```

### Metric types

| Type          | Behaviour                                            | Examples in this repo                                              |
| ------------- | ---------------------------------------------------- | ------------------------------------------------------------------ |
| **Counter**   | Only increments (resets on restart)                  | `http_requests_total`, `db_queries_total`, `db_slow_queries_total` |
| **Gauge**     | Goes up and down                                     | `db_pool_connections_total`, `db_pool_connections_idle`            |
| **Histogram** | Records values in buckets for percentile calculation | `http_request_duration_seconds`, `db_query_duration_seconds`       |

### Cardinality control

Prometheus creates one time-series per unique label combination. Using raw paths as labels causes unbounded cardinality:

```
# BAD: every table name = new time series
http_requests_total{path="/api/data_view/columns/users"} 5
http_requests_total{path="/api/data_view/columns/orders"} 3
```

This repo uses Axum's `MatchedPath` to record the route template:

```
# GOOD: bounded cardinality
http_requests_total{path="/api/data_view/columns/{table_name}"} 8
```

### Accessing metrics

```bash
curl http://localhost:8050/metrics
```

Returns Prometheus exposition format, ready to be scraped.
