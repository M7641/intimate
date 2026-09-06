# Spawner

Observability demo API — generates traces, metrics, logs, and CPU-heavy work for profiling. Hit the endpoints and watch telemetry flow through the OTel Collector into Tempo, Loki, Prometheus, and Grafana.

## Prerequisites

```bash
brew install podman          # if not already installed
podman machine init          # first time only
podman machine start         # start the VM
```

Verify with `podman machine info` — look for `machinestate: Running`.

## Quick Start

```bash
cd rose/apocrypha/spawner

# Start the full stack (pulls images on first run — ~2 GB):
uv run spawner stack

# Opens:
#   Spawner API     http://localhost:8000
#   Grafana         http://localhost:3001
```

## CLI Commands

| Command | What it does |
|---------|-------------|
| `uv run spawner stack` | `podman compose up -d` all backends + starts spawner |
| `uv run spawner run` | Starts only the spawner API (backends must be running separately) |
| `uv run spawner down` | Stops and removes all backend containers |
| `uv run spawner status` | Shows which backend containers are running |

## Example Session

```bash
# 1. Start everything
uv run spawner stack

# 2. Generate some telemetry (in another terminal)
curl http://localhost:8000/traces/nested
curl http://localhost:8000/traces/error
curl -X POST http://localhost:8000/metrics/order
curl -X POST http://localhost:8000/metrics/order
curl http://localhost:8000/logs/levels
curl http://localhost:8000/logs/structured

# 3. Run a workflow end-to-end
WF=$(curl -s -X POST "http://localhost:8000/workflows/?workflow_type=etl" \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")
for i in 1 2 3 4 5; do curl -s -X POST http://localhost:8000/workflows/$WF/step; done

# 4. CPU work (shows up in Pyroscope if SDK is configured)
curl http://localhost:8000/profile/fibonacci/30
curl http://localhost:8000/profile/sort/1000000

# 5. Open Grafana and explore
#    Dashboards → Observability folder → Web/API Services, Workflows, Infrastructure
open http://localhost:3001

# 6. When done — Ctrl+C to stop spawner, then:
uv run spawner down
```

## Endpoints

### Traces → Tempo

| Endpoint | What it generates |
|----------|-------------------|
| `GET /traces/simple` | Single span with attributes |
| `GET /traces/nested` | 3-level span tree: ingest → validate → persist |
| `GET /traces/error` | Span with recorded exception (shows as error in Tempo) |

### Metrics → Prometheus

| Endpoint | What it generates |
|----------|-------------------|
| `POST /metrics/order` | Increments `spawner.orders` counter + records value in `spawner.order.value` histogram |
| `GET /metrics/status` | Lists all metric instruments being emitted |

### Logs → Loki

| Endpoint | What it generates |
|----------|-------------------|
| `GET /logs/levels` | One log at each level (DEBUG, INFO, WARNING, ERROR) with trace correlation |
| `GET /logs/structured` | Structured log with custom fields (user_id, action, duration, records) |

### Workflows → Grafana Workflows Dashboard

| Endpoint | What it generates |
|----------|-------------------|
| `POST /workflows/` | Starts a new multi-step workflow (custom metrics: `workflow.active_count`, `workflow.queue_depth`) |
| `POST /workflows/{id}/step` | Advances one step — records `workflow.step.duration` histogram, 10% random failure rate |
| `GET /workflows/{id}` | Returns status + stalled detection (seconds since last transition) |
| `GET /workflows/` | Lists all workflows |

### Profiling → Pyroscope

| Endpoint | What it generates |
|----------|-------------------|
| `GET /profile/fibonacci/{n}` | Recursive fibonacci (CPU-bound, keep n ≤ 35) |
| `GET /profile/sort/{size}` | Sorts a random array (CPU-bound) |

## Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `OTEL_SERVICE_NAME` | `spawner` | Service name in all telemetry |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4318` | OTel Collector HTTP endpoint |
| `DEPLOYMENT_ENV` | `dev` | Added as `deployment.environment` resource attribute |

## Cleanup

The stack pulls ~2 GB of images and creates named volumes for persistent data. To reclaim disk space:

```bash
# Stop containers (if still running)
uv run spawner down

# Remove the pulled images (~2 GB)
podman rmi grafana/tempo:2.4.1 grafana/loki:2.9.4 grafana/pyroscope:1.4.0 \
  grafana/grafana:10.4.0 otel/opentelemetry-collector-contrib:0.96.0 prom/prometheus

# Remove named volumes (telemetry data)
podman volume rm apocrypha_tempo-data apocrypha_loki-data \
  apocrypha_prometheus-data apocrypha_pyroscope-data apocrypha_grafana-data

# Or nuclear option — remove ALL unused podman data
podman system prune -a --volumes

# Stop the VM when you're done for the day (frees ~2 GB RAM)
podman machine stop
```
