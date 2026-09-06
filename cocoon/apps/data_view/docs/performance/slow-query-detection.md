# Slow Query Detection

> Status: **Implemented** — `src/state.rs`

## What

Automatic identification of database queries that exceed a duration threshold (default: 1 second). Slow queries are logged at `warn` level with their SQL text and emit a dedicated Prometheus counter.

## Why

| Without slow query detection | With slow query detection |
|------------------------------|--------------------------|
| Users report "the app is slow" — you have no idea which query | Logs tell you exactly which SQL took how long |
| Performance degrades gradually — nobody notices until it's critical | `db_slow_queries_total` metric shows the trend; alert before it's critical |
| Post-incident: "something was slow last Tuesday" — grep through millions of log lines | `tracing::warn!` with `sql` field → filter `elapsed_ms > 1000` → instant results |
| No feedback loop for query optimisation | Slow query log is a prioritised list of what to optimise next |

Slow queries are the #1 cause of service degradation in database-backed applications. Detecting them is the first step to fixing them.

## How — This Repo

### Detection in `blocking_query` (`src/state.rs`)

```rust
const SLOW_QUERY_THRESHOLD_MS: u64 = 1000;

let elapsed = start.elapsed();
let elapsed_ms = elapsed.as_millis() as u64;

if elapsed_ms > SLOW_QUERY_THRESHOLD_MS {
    tracing::warn!(
        sql = %sql_ref,
        elapsed_ms,
        threshold_ms = SLOW_QUERY_THRESHOLD_MS,
        "Slow query detected"
    );
    metrics::counter!("db_slow_queries_total").increment(1);
}
```

### What gets recorded

Every query emits:

| Metric | Always | Slow only |
|--------|--------|-----------|
| `db_query_duration_seconds` (histogram) | Yes | — |
| `db_queries_total{status}` (counter) | Yes | — |
| `db_slow_queries_total` (counter) | — | Yes |
| `tracing::warn!` with SQL text | — | Yes |

The histogram captures the full distribution (including fast queries) for percentile calculations. The slow counter is a quick signal for alerting.

### Why 1 second?

- Most data exploration queries (column stats, row counts, schemas) should complete in <100ms
- A 1-second threshold catches genuinely problematic queries without noise from normal variance
- Adjust the `SLOW_QUERY_THRESHOLD_MS` constant based on your workload

### Using the data

**Find the worst offenders:**

```bash
# In logs (development)
RUST_LOG="data_view::state=warn" ./dv start
# All slow query warnings include the SQL text

# In Prometheus
topk(5, rate(db_slow_queries_total[1h]))
```

**Alert on sustained slow queries:**

```yaml
- alert: SlowQuerySpike
  expr: rate(db_slow_queries_total[5m]) > 0.1
  for: 5m
  annotations:
    summary: "More than 6 slow queries per minute for 5 minutes"
```
