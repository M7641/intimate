# Collector & Backend Setup

Local observability stack: OTel Collector + Grafana Tempo + Grafana, deployed via `docker-compose`.

## Why Use the Collector

You *could* export directly from each service to Tempo/Prometheus, but the Collector gives you:

- **Vendor-agnostic** — switch backends without changing application code
- **Batching & retry** — reduces network overhead and handles transient failures
- **Tail sampling** — decide to keep/drop a trace after seeing all its spans (impossible in-app)
- **Processing** — filter out `/health` spans, redact PII, add resource attributes
- **Single endpoint** — all services point at `localhost:4317`, Collector routes to the right backend

## Docker Compose

Save as `apocrypha/otel/docker-compose.yml`:

```yaml
version: "3.9"

services:
  # ──────────────────────────────────────────────
  # OpenTelemetry Collector
  # ──────────────────────────────────────────────
  otel-collector:
    image: otel/opentelemetry-collector-contrib:0.96.0
    command: ["--config=/etc/otel/config.yaml"]
    volumes:
      - ./otel-collector-config.yaml:/etc/otel/config.yaml:ro
    ports:
      - "4317:4317"   # OTLP gRPC receiver (Rust services use this)
      - "4318:4318"   # OTLP HTTP receiver  (Python services use this)
      - "8888:8888"   # Collector own metrics (Prometheus can scrape this)
      - "8889:8889"   # Prometheus exporter  (exposes app metrics for Prometheus)
    depends_on:
      - tempo

  # ──────────────────────────────────────────────
  # Grafana Tempo (distributed tracing backend)
  # ──────────────────────────────────────────────
  tempo:
    image: grafana/tempo:2.4.1
    command: ["-config.file=/etc/tempo/tempo.yaml"]
    volumes:
      - ./tempo-config.yaml:/etc/tempo/tempo.yaml:ro
      - tempo-data:/var/tempo
    ports:
      - "3200:3200"   # Tempo query frontend (Grafana connects here)

  # ──────────────────────────────────────────────
  # Grafana (dashboards + trace explorer)
  # ──────────────────────────────────────────────
  grafana:
    image: grafana/grafana:10.4.0
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Admin
      - GF_AUTH_DISABLE_LOGIN_FORM=true
    volumes:
      - ./grafana-datasources.yaml:/etc/grafana/provisioning/datasources/datasources.yaml:ro
      - grafana-data:/var/lib/grafana
    ports:
      - "3001:3000"   # Port 3001 to avoid conflicts with flow/tako on 3000
    depends_on:
      - tempo

volumes:
  tempo-data:
  grafana-data:
```

## Collector Configuration

Save as `apocrypha/otel/otel-collector-config.yaml`:

```yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

processors:
  # Always batch — never send spans one at a time
  batch:
    timeout: 5s
    send_batch_size: 512
    send_batch_max_size: 1024

  # Drop noisy health check spans
  filter:
    error_mode: ignore
    traces:
      span:
        - 'attributes["http.route"] == "/health"'
        - 'attributes["http.target"] == "/health"'

  # Add resource attributes to everything passing through
  resource:
    attributes:
      - key: deployment.environment
        value: local
        action: upsert

exporters:
  # Send traces to Tempo
  otlp/tempo:
    endpoint: tempo:3200
    tls:
      insecure: true

  # Expose metrics for Prometheus to scrape
  prometheus:
    endpoint: 0.0.0.0:8889
    resource_to_telemetry_conversion:
      enabled: true

  # Debug: log telemetry to collector stdout (disable in prod)
  debug:
    verbosity: basic

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: [filter, batch, resource]
      exporters: [otlp/tempo, debug]
    metrics:
      receivers: [otlp]
      processors: [batch, resource]
      exporters: [prometheus, debug]
```

## Tempo Configuration

Save as `apocrypha/otel/tempo-config.yaml`:

```yaml
server:
  http_listen_port: 3200

distributor:
  receivers:
    otlp:
      protocols:
        grpc:
          endpoint: 0.0.0.0:4317

storage:
  trace:
    backend: local
    local:
      path: /var/tempo/traces
    wal:
      path: /var/tempo/wal

# Keep traces for 72 hours in local dev
compactor:
  compaction:
    block_retention: 72h
```

## Grafana Data Source Provisioning

Save as `apocrypha/otel/grafana-datasources.yaml`:

```yaml
apiVersion: 1

datasources:
  # Tempo for traces
  - name: Tempo
    type: tempo
    access: proxy
    url: http://tempo:3200
    isDefault: true
    jsonData:
      tracesToLogsV2:
        datasourceUid: ""
      serviceMap:
        datasourceUid: ""

  # Prometheus for metrics (existing setup + OTel Collector metrics)
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://host.docker.internal:8050
    jsonData:
      timeInterval: "5s"
```

## Integration with Existing Prometheus

The existing `prometheus/prometheus.yml` scrapes `red_db`'s `/prometheus/metrics`. To also scrape OTel Collector metrics, add a job:

```yaml
# Add to prometheus/prometheus.yml scrape_configs:
- job_name: 'otel-collector'
  scrape_interval: 15s
  static_configs:
    - targets: ['localhost:8889']
  metrics_path: /metrics
```

This gives Prometheus access to any OTel metrics exported by the Collector's `prometheus` exporter, alongside the existing `red_db` metrics.

## Running the Stack

```bash
# From apocrypha/otel/
docker compose up -d

# Verify everything is healthy
docker compose ps

# View collector logs (useful for debugging)
docker compose logs -f otel-collector

# Access Grafana
open http://localhost:3001
```

### Verify Connectivity

1. **Collector is receiving**: Check `http://localhost:8888/metrics` — look for `otelcol_receiver_accepted_spans`
2. **Tempo has traces**: In Grafana → Explore → Tempo → Search → run an empty query
3. **Prometheus exporter works**: Check `http://localhost:8889/metrics` for OTel metrics

## Ports Summary

| Port | Service | Protocol | Purpose |
|------|---------|----------|---------|
| 4317 | Collector | gRPC | OTLP receiver (Rust services) |
| 4318 | Collector | HTTP | OTLP receiver (Python services) |
| 8888 | Collector | HTTP | Collector internal metrics |
| 8889 | Collector | HTTP | Prometheus exporter (app metrics) |
| 3200 | Tempo | HTTP | Query frontend |
| 3001 | Grafana | HTTP | Dashboards & trace explorer |

## Next Steps

- [03-fastapi.md](03-fastapi.md) — configure Python services to export to `localhost:4318`
- [04-axum.md](04-axum.md) — configure Rust services to export to `localhost:4317`
