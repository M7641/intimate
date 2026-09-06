# Load Testing

> Status: **Planned**

## What

Simulating realistic traffic patterns against the service to validate performance, identify bottlenecks, and establish baselines before production deployment.

## Why

| Without load testing | With load testing |
|---------------------|------------------|
| "It works on my machine" — with 1 concurrent user | Validated at 50-100 concurrent users matching expected production load |
| Pool size is a guess (default 10) | Pool size is calibrated: too small causes queuing, too large wastes DB connections |
| Slow queries are discovered by users | Slow endpoints are identified and optimised before deployment |
| No performance baselines | Regressions are detected by comparing against known benchmarks |
| Capacity planning is guesswork | Data-driven decisions: "we need X instances to serve Y users" |

Load testing is the only way to validate that your connection pool, timeouts, and infrastructure can handle real-world traffic. Everything else is theory.

## How — Implementation Plan

### Tool: `oha` (or `wrk`, `k6`)

`oha` is a Rust-based HTTP load testing tool with excellent latency reporting:

```bash
# Install
cargo install oha

# Basic load test: 100 concurrent connections for 30 seconds
oha -c 100 -z 30s http://localhost:8050/api/data_view/schemas

# Test a specific endpoint with query parameters
oha -c 50 -z 60s "http://localhost:8050/api/data_view/data/my_table?limit=100"
```

### Key scenarios to test

| Scenario | Command | What to watch |
|----------|---------|---------------|
| Baseline throughput | `oha -c 10 -z 30s /api/data_view/schemas` | req/s, P95 latency |
| Pool saturation | `oha -c 50 -z 60s /api/data_view/data/{table}` | `db_pool_connections_idle` dropping to 0 |
| Error rate under load | `oha -c 100 -z 120s /api/data_view/columns/{table}` | `http_requests_total{status="500"}` |
| Slow endpoint | `oha -c 20 -z 60s /api/data_view/column_values/{table}/{col}` | P99 latency, `db_slow_queries_total` |

### What to measure

| Metric | Acceptable | Investigate |
|--------|-----------|-------------|
| P95 latency | < 500ms | > 1s |
| P99 latency | < 2s | > 5s |
| Error rate | < 0.1% | > 1% |
| Pool utilisation | < 80% | > 90% |
| Throughput | Stable | Declining over time (indicates resource leak) |

### When to run

- Before every production deployment (CI pipeline)
- After changing pool configuration
- After adding new endpoints or changing query patterns
- When scaling infrastructure up or down

### Correlating with metrics

During a load test, monitor the Prometheus metrics in real-time:

```bash
# In terminal 1: run the load test
oha -c 50 -z 60s http://localhost:8050/api/data_view/schemas

# In terminal 2: watch key metrics
watch -n 1 'curl -s http://localhost:8050/metrics | grep -E "(http_requests_total|db_pool|db_slow)"'
```

This shows pool behaviour, error rates, and slow queries as the load increases — exactly the data you need to tune pool size and timeout values.
