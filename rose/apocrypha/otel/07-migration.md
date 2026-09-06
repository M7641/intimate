# Phased Migration Plan

Rolling out OpenTelemetry across the monorepo in six phases. Each phase is independently valuable — you get benefits after each one, not just at the end.

**Key principle:** OTel is additive. Existing logging (`tracing`, `NimbusLogger`) and metrics (`prometheus-fastapi-instrumentator`) continue working unchanged. Nothing is removed until the OTel equivalent is validated.

## Phase 1: Infrastructure

**Goal:** Local observability stack running in Docker.

**Steps:**
1. Copy the config files from [02-collector-setup.md](02-collector-setup.md) into `apocrypha/otel/`:
   - `docker-compose.yml`
   - `otel-collector-config.yaml`
   - `tempo-config.yaml`
   - `grafana-datasources.yaml`
2. Run `docker compose up -d` from `apocrypha/otel/`
3. Verify: Grafana accessible at `http://localhost:3001`, Tempo data source connected

**Deliverable:** `docker compose up` spins up Collector + Tempo + Grafana with zero service changes.

**Rollback:** `docker compose down -v` — nothing touches application code.

## Phase 2: First Rust Service — `flow`

**Why `flow` first:** Simplest Axum service. Already has `tracing` + `tracing-subscriber`. No graceful shutdown to worry about (we add a minimal one). No existing metrics to coordinate with.

**Steps:**
1. Add dependencies to `rose/flow/Cargo.toml`:
   ```toml
   tracing-opentelemetry = "0.29"
   opentelemetry = "0.28"
   opentelemetry_sdk = { version = "0.28", features = ["rt-tokio"] }
   opentelemetry-otlp = { version = "0.28", features = ["tonic"] }
   tower-http = { version = "0.6", features = ["trace", "cors"] }
   ```
2. Replace `tracing_subscriber::fmt().init()` with layered registry + OTel layer (see [04-axum.md](04-axum.md))
3. Add `TraceLayer::new_for_http()` to the router
4. Add graceful shutdown to flush spans on exit
5. Verify: make requests → see traces in Grafana Tempo under `service.name = "flow"`

**Deliverable:** `flow` traces visible in Tempo. Console output unchanged.

**Rollback:** Remove the `otel_layer` from the registry and the OTel deps. `fmt::layer()` still works.

## Phase 3: First Python Service — `red_db`

**Why `red_db` first:** Already has Prometheus (`prometheus-fastapi-instrumentator`), so it's the most complex integration case. If OTel works cleanly alongside Prometheus here, it'll work everywhere.

**Steps:**
1. Add OTel dependencies to `cocoon/red_db/pyproject.toml`
2. Create `src/red_db/otel.py` with the `setup_otel()` function (see [03-fastapi.md](03-fastapi.md))
3. Call `setup_otel(app)` in the FastAPI lifespan
4. Instrument outgoing `httpx` calls: `HTTPXClientInstrumentor().instrument()`
5. Optionally update `NimbusLogger` format string to include `trace_id` (see [03-fastapi.md](03-fastapi.md))
6. Verify: make requests → see traces in Tempo under `service.name = "red_db"`
7. Verify: existing Prometheus metrics at `/prometheus/metrics` still work

**Deliverable:** `red_db` traces in Tempo. Prometheus unaffected. Log lines include trace IDs.

**Rollback:** Remove `setup_otel()` call from lifespan. OTel packages stay installed but dormant (no-op).

## Phase 4: Cross-Service Validation — `red_db → tako`

**Why:** This validates the entire distributed tracing pipeline: Python → Rust with trace context propagation.

**Steps:**
1. Instrument `tako` (same pattern as Phase 2, plus context extraction middleware)
2. Add context propagation middleware to `tako` (see [05-cross-service.md](05-cross-service.md))
3. Add `#[tracing::instrument]` to key `tako` handlers
4. Add OTel shutdown to `tako`'s existing graceful shutdown handler
5. Trigger a request from `red_db` to `tako`
6. Verify in Grafana Tempo: one trace with spans from both `red_db` and `tako`

**Deliverable:** A single trace in Tempo showing the full `red_db → tako` request flow.

**Rollback:** Remove OTel layer from `tako`. `fmt::layer()` + graceful shutdown still work.

## Phase 5: Remaining Services

Instrument the rest in any order:

| Service | Notes |
|---------|-------|
| `snow_db` | Same pattern as `red_db` but simpler (no Prometheus). Add manual spans for Snowflake queries. |
| `obscur` | Needs `tracing` + `tracing-subscriber` added from scratch (currently `eprintln!` only). See [04-axum.md](04-axum.md) "Bootstrap from Scratch" section. |
| `app_one` | Standard FastAPI pattern from [03-fastapi.md](03-fastapi.md). |
| `app_two` | Standard FastAPI pattern. |
| `react_minimal` | Standard FastAPI pattern. |

**Deliverable:** All services visible in Grafana Tempo. Full service map available.

## Phase 6: Legacy Cleanup (Optional)

Only after OTel equivalents are validated and trusted:

| Legacy | OTel Replacement | Action |
|--------|-----------------|--------|
| `prometheus-fastapi-instrumentator` in `red_db` | OTel `MeterProvider` + Collector Prometheus exporter | Remove `Instrumentator()` from `api.py`. Metrics flow through OTel → Collector → Prometheus instead. |
| `NimbusLogger` ANSI format | Structured JSON format for Loki ingestion | Update `NimbusFormatter` to output JSON. Loki can parse and index `trace_id` fields automatically. |
| `console-subscriber` dep in `tako` | OTel traces in Tempo | Remove from `Cargo.toml` (it was never wired in). |

**Important:** Phase 6 is not urgent. The existing tools work fine alongside OTel. Only migrate when you're confident in the OTel pipeline and ready to reduce dependency count.

## Timeline Estimate

| Phase | Scope | Blocking? |
|-------|-------|-----------|
| 1. Infrastructure | Docker config only | No code changes |
| 2. `flow` (Rust) | One service | Independent |
| 3. `red_db` (Python) | One service | Independent of Phase 2 |
| 4. Cross-service | Requires Phase 3 + `tako` instrumented | Depends on Phase 3 |
| 5. Remaining | Five services | Independent of each other |
| 6. Legacy cleanup | Optional | Depends on all previous phases validated |

Phases 2 and 3 can be done in parallel by different people. Phase 4 is the key validation milestone.

## Rollback Strategy

At every phase, rollback is the same:

1. **Application code:** Remove or comment out the OTel layer/setup call. Existing logging and metrics are untouched.
2. **Infrastructure:** `docker compose down -v` removes Collector, Tempo, and Grafana with all data.
3. **Dependencies:** OTel packages can stay installed — without `setup_otel()` or the OTel layer, all OTel API calls are safe no-ops.

The worst case is "traces stop appearing in Grafana." No existing functionality is degraded.
