# Alerting & Dashboards

## What

Alerting rules define conditions that trigger notifications (PagerDuty, Slack, email) when metrics cross thresholds. Dashboards visualise metrics in real-time for situational awareness.

## Why

Metrics without alerts are data that nobody looks at until it's too late. Dashboards without alerts rely on someone staring at a screen. Alerts without dashboards send notifications with no context.

**You need both:**

- **Alerts** catch problems before users do. A 5-minute error-rate spike at 3am triggers a page, not a morning support ticket.
- **Dashboards** provide context during an incident. "The alert says error rate is high — the dashboard shows it started when pool utilisation hit 100%."

Without these, your metrics are a write-only system.

## Key Grafana Dashboard Panels

Using the metrics exposed by this service at `GET /metrics`:

### 1. Request Rate (req/s)

```promql
sum(rate(http_requests_total[5m])) by (path)
```

Shows throughput per endpoint. Sudden drops indicate an outage; spikes may indicate a traffic surge or retry storm.

### 2. Error Rate (%)

```promql
sum(rate(http_requests_total{status=~"5.."}[5m]))
/ sum(rate(http_requests_total[5m])) * 100
```

The single most important panel. Target: < 0.1% under normal conditions.

### 3. P50 / P95 / P99 Latency

```promql
histogram_quantile(0.95, sum(rate(http_request_duration_seconds_bucket[5m])) by (le, path))
```

P95 is the standard SLO target. P99 catches tail latency that averages hide.

### 4. Slow Queries per Minute

```promql
rate(db_slow_queries_total[1m]) * 60
```

Correlate with deployment timestamps to identify regressions.

### 5. Pool Utilisation (%)

```promql
(db_pool_connections_total - db_pool_connections_idle) / db_pool_connections_total * 100
```

When this approaches 100%, new queries queue waiting for a connection. Alert before it gets there.

## Alerting Rules (Prometheus Alertmanager)

```yaml
groups:
  - name: data-view-alerts
    rules:
      - alert: HighErrorRate
        expr: >
          sum(rate(http_requests_total{status=~"5.."}[5m]))
          / sum(rate(http_requests_total[5m])) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Error rate above 5% for 5 minutes"

      - alert: HighP95Latency
        expr: >
          histogram_quantile(0.95,
            rate(http_request_duration_seconds_bucket[5m])
          ) > 2
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "P95 latency above 2 seconds"

      - alert: PoolExhaustion
        expr: db_pool_connections_idle == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "No idle DB connections — queries will queue"

      - alert: SlowQuerySpike
        expr: rate(db_slow_queries_total[5m]) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Sustained slow query rate detected"
```

## SLO Guidance

| Signal                 | Target                | Reasoning                                                          |
| ---------------------- | --------------------- | ------------------------------------------------------------------ |
| Availability (non-5xx) | 99.9%                 | ~8.7 hours of downtime/year — reasonable for internal tooling      |
| P95 latency            | < 500ms               | Most endpoints query a single table; 500ms allows for network + DB |
| P99 latency            | < 2s                  | Catches pathological cases (cold pool, large scans)                |
| Error budget burn rate | < 2x in any 1h window | Triggers investigation before the monthly budget is exhausted      |
