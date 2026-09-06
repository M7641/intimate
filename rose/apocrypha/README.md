# Apocrypha

Observability stack for the monorepo — metrics, traces, logs, and profiling.

## Architecture

```
Applications (spawner, flow, tako, red_db, etc.)
       │
       │  OTLP gRPC :4317 (Rust)
       │  OTLP HTTP :4318 (Python)
       ▼
┌──────────────────────────┐
│     OTel Collector       │
│  (:4317, :4318, :8889)   │
├──────────────────────────┤
│ traces  ──→ Tempo (:3200)│
│ logs    ──→ Loki  (:3100)│
│ metrics ──→ Prometheus   │
│            (scrape :8889)│
└──────────────────────────┘
                              Pyroscope (:4040) ← direct from apps
       │
       └── all backends ──→ Grafana (:3001)
```

## Quick Start

```bash
cd rose/apocrypha/spawner
podman machine start
uv run spawner stack        # starts all backends + demo API
uv run spawner flood        # generates traffic (second terminal)
open http://localhost:3001   # Grafana dashboards
uv run spawner down         # tear down
```

## Structure

```
apocrypha/
├── docker-compose.yaml          # orchestrates all backends
├── prometheus-stack.yml         # prometheus config for local stack
├── otel-collector/              # config + Dockerfile
├── tempo/                       # config + Dockerfile
├── loki/                        # config + Dockerfile
├── pyroscope/                   # config + Dockerfile
├── grafana/                     # config + Dockerfile + dashboards
├── prometheus/                  # config + Dockerfile + deploy CLI
├── spawner/                     # demo FastAPI app (uv run spawner)
└── otel/                        # instrumentation guides
```

## Services

| Service        | Port(s)          | Signal               |
| -------------- | ---------------- | -------------------- |
| OTel Collector | 4317, 4318, 8889 | Routes all telemetry |
| Tempo          | 3200             | Traces               |
| Loki           | 3100             | Logs                 |
| Prometheus     | 8050             | Metrics              |
| Pyroscope      | 4040             | Profiling            |
| Grafana        | 3001             | Dashboards           |
| Spawner        | 8000             | Demo API             |

## Guides

- **[`otel/`](otel/00-overview.md)** — OpenTelemetry instrumentation guides for FastAPI and Axum services.
