# OpenTelemetry Overview

End-to-end observability for the monorepo — traces, metrics, and logs unified through OpenTelemetry.

## Current Observability State

| Service | Language | Logging | Metrics | Tracing | OTel |
|---------|----------|---------|---------|---------|------|
| `rose/flow` | Rust/Axum | `tracing` + `fmt` subscriber | None | None | None |
| `cocoon/tako` | Rust/Axum | `tracing` + layered registry | None | None | None |
| `rose/obscur` | Rust/Axum | `eprintln!` only | None | None | None |
| `cocoon/red_db` | Python/FastAPI | `NimbusLogger` (stdlib) | Prometheus (`/prometheus/metrics`) | None | None |
| `cocoon/snow_db` | Python/FastAPI | `NimbusLogger` (stdlib) | None | None | None |
| `blank/app_one` | Python/FastAPI | `NimbusLogger` (stdlib) | None | None | None |
| `blank/app_two` | Python/FastAPI | `NimbusLogger` (stdlib) | None | None | None |
| `blank/react_minimal` | Python/FastAPI | `NimbusLogger` (stdlib) | None | None | None |

### Key Gaps

- **Zero distributed tracing** — no service exports OTLP spans; no request can be followed across boundaries
- **No trace context propagation** — no W3C `traceparent` headers between services
- **No log correlation** — logs cannot be linked to a specific trace or span
- **Fragmented metrics** — only `red_db` has Prometheus; all other services are blind
- **`obscur` has no instrumentation at all** — raw `eprintln!` to stderr
- **NimbusLogger outputs ANSI color, not structured JSON** — incompatible with log aggregation (Loki) without changes

## Target Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                         Applications                             │
│                                                                  │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌───────┐ │
│  │  flow   │  │  tako   │  │ obscur  │  │ red_db  │  │snow_db│ │
│  │  (Rust) │  │  (Rust) │  │  (Rust) │  │(Python) │  │(Py)   │ │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘  └───┬───┘ │
│       │            │            │             │            │     │
│       │     OTel SDK (traces, metrics, logs)  │            │     │
│       │      tracing-opentelemetry layer      │            │     │
│       └────────────┼────────────┼─────────────┼────────────┘     │
│                    │            │             │                   │
└────────────────────┼────────────┼─────────────┼──────────────────┘
                     │   OTLP/gRPC (4317)       │
                     │   OTLP/HTTP (4318)       │
                     ▼            ▼             ▼
            ┌────────────────────────────────────────┐
            │         OpenTelemetry Collector         │
            │                                        │
            │  Receivers ─► Processors ─► Exporters  │
            │  (OTLP)       (batch,        (OTLP,   │
            │                filter)        prom)    │
            └───────┬──────────┬──────────────┬──────┘
                    │          │              │
         ┌──────────┘    ┌─────┘        ┌─────┘
         ▼               ▼              ▼
   ┌───────────┐  ┌───────────┐  ┌───────────┐
   │  Grafana  │  │  Grafana  │  │Prometheus │
   │   Tempo   │  │   Loki    │  │  (scrape  │
   │  (traces) │  │  (logs)   │  │  endpoint)│
   └─────┬─────┘  └─────┬─────┘  └─────┬─────┘
         │               │              │
         └───────────────┼──────────────┘
                         ▼
                  ┌─────────────┐
                  │   Grafana   │
                  │ (dashboards,│
                  │  trace view,│
                  │  log search)│
                  └─────────────┘
```

## Guide Map

Read in order — each guide builds on the previous:

```
00-overview.md          ◄── You are here
    │
    ▼
01-fundamentals.md      Core OTel concepts (traces, metrics, logs, propagation)
    │
    ▼
02-collector-setup.md   Infrastructure: Collector + Tempo + Grafana (docker-compose)
    │
    ├──────────────────────────────┐
    ▼                              ▼
03-fastapi.md                 04-axum.md
Python/FastAPI                Rust/Axum
instrumentation               instrumentation
    │                              │
    └──────────┬───────────────────┘
               ▼
    05-cross-service.md     Distributed tracing across Python ↔ Rust
               │
               ▼
    06-production.md        Sampling, performance, security, cost
               │
               ▼
    07-migration.md         Phased rollout plan for the monorepo
```

Guides `03` and `04` can be read in either order (or in parallel by different team members).

## Glossary

| Term | Definition |
|------|-----------|
| **Span** | A unit of work with a name, start time, duration, and attributes. A single HTTP request handler creates one span. |
| **Trace** | A tree of spans sharing a single trace ID. A request flowing through `red_db → tako` produces one trace with spans from both services. |
| **Trace Context** | The W3C standard for propagating trace identity across service boundaries via the `traceparent` HTTP header. |
| **Resource** | Metadata describing the source of telemetry — `service.name`, `service.version`, `deployment.environment`. |
| **Scope** | The instrumentation library or module that created a span (e.g., `opentelemetry-instrumentation-fastapi`). |
| **Exporter** | Component that sends telemetry data to a backend. OTLP is the standard protocol. |
| **Propagator** | Injects/extracts trace context into/from carriers (HTTP headers). |
| **Collector** | A vendor-agnostic proxy that receives, processes, and exports telemetry. Decouples apps from backends. |
| **OTLP** | OpenTelemetry Protocol — the native wire format for traces, metrics, and logs over gRPC (port 4317) or HTTP (port 4318). |
| **BatchSpanProcessor** | Buffers spans and sends them in batches to reduce overhead. Always use in production (never `SimpleSpanProcessor`). |
| **Sampling** | Controls what percentage of traces are recorded. Head-based decides at trace start; tail-based decides after the trace completes. |
