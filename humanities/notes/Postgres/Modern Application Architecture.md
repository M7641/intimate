
# Modern Data-Driven ML Application Architecture

## The Problem Space

You want to build an application that:

1. Serves a product (API or web app) to end users with low latency and high throughput.
2. Runs ML models as part of the product experience (recommendations, predictions, classification, generation, etc.).
3. Allows non-technical or semi-technical users to configure the ML behaviour, feature flags, model parameters, and business rules through a UI.
4. Collects, stores, and processes data at scale for both operational and analytical purposes.
5. Doesn't collapse under its own complexity.

What follows is a reference architecture that synthesises the PostgreSQL/Snowflake duality with modern ML serving, event-driven patterns, and a configuration plane — optimised for throughput and UX.

---

## High-Level Architecture

The system decomposes into **five planes**, each with a clear responsibility boundary:

```
┌─────────────────────────────────────────────────────────────────┐
│                     1. EXPERIENCE PLANE                         │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │  Web App UI  │  │  Public API  │  │  Config/Admin UI      │  │
│  │  (React/Next)│  │  (REST/gRPC) │  │  (Feature flags, ML   │  │
│  │              │  │              │  │   params, rules)       │  │
│  └──────┬───────┘  └──────┬───────┘  └───────────┬───────────┘  │
└─────────┼─────────────────┼──────────────────────┼──────────────┘
          │                 │                      │
┌─────────┼─────────────────┼──────────────────────┼──────────────┐
│         ▼                 ▼                      ▼              │
│                     2. SERVICE PLANE                            │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │  App Service  │  │  ML Gateway  │  │  Config Service       │  │
│  │  (Axum/Fast-  │  │  (routing,   │  │  (CRUD rules, params, │  │
│  │   API)        │  │   A/B, fan-  │  │   flags → PostgreSQL) │  │
│  │              │  │   out)       │  │                       │  │
│  └──────┬───────┘  └──────┬───────┘  └───────────┬───────────┘  │
│         │                 │                      │              │
│         │          ┌──────┴───────┐              │              │
│         │          │  ML Serving  │              │              │
│         │          │  (dedicated  │              │              │
│         │          │   inference) │              │              │
│         │          └──────┬───────┘              │              │
└─────────┼─────────────────┼──────────────────────┼──────────────┘
          │                 │                      │
┌─────────┼─────────────────┼──────────────────────┼──────────────┐
│         ▼                 ▼                      ▼              │
│                     3. DATA PLANE                               │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │  PostgreSQL   │  │  Feature     │  │  Event Bus            │  │
│  │  (operational │  │  Store       │  │  (NATS / Kafka)       │  │
│  │   state)      │  │  (Redis)     │  │                       │  │
│  └──────┬───────┘  └──────────────┘  └───────────┬───────────┘  │
└─────────┼────────────────────────────────────────┼──────────────┘
          │                                        │
┌─────────┼────────────────────────────────────────┼──────────────┐
│         ▼                                        ▼              │
│                  4. ANALYTICAL PLANE                             │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │  Snowflake    │  │  ML Training │  │  Orchestrator         │  │
│  │  (warehouse)  │  │  Pipeline    │  │  (Airflow/Dagster)    │  │
│  └──────────────┘  └──────────────┘  └───────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
          │
┌─────────┼───────────────────────────────────────────────────────┐
│         ▼           5. CONFIGURATION PLANE                      │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │  Model        │  │  Feature     │  │  A/B & Experiment     │  │
│  │  Registry     │  │  Flags       │  │  Config               │  │
│  └──────────────┘  └──────────────┘  └───────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Plane-by-Plane Breakdown

### 1. Experience Plane — What Users Touch

This is the layer that determines UX quality. The goal is **perceived speed**: every interaction should feel instant, even if work is happening asynchronously behind the scenes.

**Public-facing Web App or API**

|Concern|Approach|
|---|---|
|Framework|Next.js (web) or a thin API gateway (Kong, Envoy) in front of backend services|
|Rendering strategy|Server-side render the shell, stream ML-powered content into slots via React Server Components or SSE|
|Optimistic updates|Apply user actions immediately in the UI; reconcile with server response asynchronously|
|Caching|Edge cache (Cloudflare/Vercel) for static content; stale-while-revalidate for semi-dynamic|
|Real-time updates|WebSocket or SSE for live data (e.g., prediction results streaming in); NATS → WebSocket bridge for backend events|

**Configuration / Admin UI**

This is the UI where operators configure the ML system. It needs to feel like a product, not a developer tool.

What it controls:

- **Feature flags** — toggle features, enable/disable models, route traffic.
- **Model parameters** — confidence thresholds, temperature, top-k, business rule overrides.
- **A/B experiment config** — traffic splits, variant definitions, success metrics.
- **Data pipeline config** — source selection, transformation rules, scheduling.
- **Alerting rules** — define thresholds for model drift, latency, error rates.

Architecture:

- React frontend with a form-driven UI backed by JSON Schema (so the config UI auto-generates from model metadata).
- Writes to the Config Service, which persists to PostgreSQL and publishes change events to NATS.
- All downstream services subscribe to config change events and hot-reload without restart.
- Every config change is versioned (audit trail in PostgreSQL) and can be rolled back.

**Why this matters for UX:** If a product manager wants to change a model threshold from 0.7 to 0.8, they click a slider, hit save, and within seconds every inference request uses the new threshold. No deployment, no ticket, no waiting.

---

### 2. Service Plane — Where Logic Lives

This is the core application layer. The key architectural decision here is **separating the ML inference path from the application logic path**, connected by an ML Gateway that abstracts model details from the rest of the system.

**App Service (Axum or FastAPI)**

Handles business logic, authentication, authorisation, request validation, and orchestration.

|Design choice|Rationale|
|---|---|
|Axum (Rust) for latency-critical paths|Predictable p99 latency, no GC pauses, excellent for high-throughput API serving|
|FastAPI (Python) for ML-adjacent services|Native Python ecosystem access, easy integration with ML libraries, great for prototyping|
|gRPC between internal services|Binary protocol, schema-enforced contracts, streaming support, ~10x faster than REST for internal calls|
|NATS for async communication|Lightweight pub/sub and request-reply; lower operational overhead than Kafka for most patterns|

The app service never calls ML models directly. It calls the ML Gateway.

**ML Gateway**

This is the critical abstraction layer. It sits between business logic and model inference and handles:

- **Routing** — Which model serves this request? Based on config (A/B split, feature flags, user segment).
- **Feature assembly** — Pulls pre-computed features from the Feature Store (Redis) and assembles the input vector.
- **Fan-out** — If the request needs multiple models (e.g., a recommendation + a safety classifier), it fires them in parallel.
- **Fallback** — If the primary model times out or errors, falls back to a simpler model or cached result.
- **Caching** — Identical inference requests within a TTL window return cached results (saves compute and latency).
- **Logging** — Every inference request/response is logged to the event bus for downstream analysis.

```
Request → ML Gateway
              │
              ├─ Read config (which model, threshold, variant)
              ├─ Assemble features from Redis
              ├─ Route to Model A (70%) or Model B (30%)
              ├─ Call inference endpoint (gRPC)
              ├─ Apply business rules (threshold, filters)
              ├─ Log to NATS (async)
              └─ Return result
```

**ML Serving (Inference)**

The actual model execution. Options:

|Option|When to use|
|---|---|
|**vLLM / TGI**|LLM serving — batched inference, KV-cache, speculative decoding|
|**Triton Inference Server**|Multi-framework (PyTorch, TensorFlow, ONNX), dynamic batching, GPU scheduling|
|**Custom Rust service (ort / candle)**|When you need absolute minimum latency and control; ONNX Runtime via ort crate or candle for pure Rust inference|
|**BentoML / Ray Serve**|Python-native, good for teams that want to stay in Python end-to-end|
|**SageMaker / Vertex AI endpoints**|Managed; if you don't want to run inference infrastructure|

**Key throughput patterns for inference:**

1. **Dynamic batching** — Accumulate requests over a small window (5-10ms) and batch them into a single GPU call. Triton and vLLM do this natively. This can increase throughput 5-20x.
2. **Model sharding** — For large models, shard across multiple GPUs with tensor parallelism.
3. **Quantisation** — INT8/INT4 quantisation (via GPTQ, AWQ, or ONNX quantise) reduces memory and increases throughput at a small accuracy cost.
4. **Async inference** — For non-latency-critical predictions, push to a queue and process in batch. Return results via webhook or SSE.
5. **Precomputation** — If predictions depend on slowly-changing data, pre-compute results on a schedule and serve from cache rather than running inference on every request.

---

### 3. Data Plane — The Transactional Core

This is where the PostgreSQL vs Snowflake decision from the previous document directly applies.

**PostgreSQL — Operational State**

PostgreSQL is the system of record for everything that needs ACID guarantees and low-latency access:

- User accounts, sessions, permissions
- Configuration state (model params, feature flags, experiment definitions)
- Application entities (orders, products, content, whatever your domain is)
- Inference audit log (short-term, hot storage — last 7 days)
- Feature flag state and experiment assignments

Schema design tips for ML applications:

- Store experiment assignments in a dedicated table (user_id, experiment_id, variant, assigned_at) — this is your ground truth for A/B analysis.
- Use JSONB columns for flexible model metadata and config payloads — avoids constant schema migrations as models evolve.
- Partition the inference log table by date — makes it easy to drop old data and keeps queries fast.

**Redis — Feature Store (Online)**

The online feature store is the bridge between the data pipeline and real-time inference. Redis (or DragonflyDB for higher throughput) stores pre-computed feature vectors keyed by entity ID.

```
Key: features:user:12345
Value: {
  "avg_session_duration_7d": 342.5,
  "purchase_count_30d": 7,
  "preferred_category": "electronics",
  "embedding_v2": [0.12, -0.34, ...],
  "last_computed": "2026-02-18T10:30:00Z"
}
```

Features are computed in the analytical plane (Snowflake → feature pipeline → Redis) on a schedule or triggered by events. At inference time, the ML Gateway reads from Redis (sub-millisecond) rather than computing features on the fly.

**NATS — Event Bus**

Every significant event flows through NATS:

- User actions (clicks, purchases, page views)
- Inference requests and responses (for monitoring and retraining)
- Config changes (feature flags, model swaps, threshold updates)
- System events (model deployed, pipeline completed, drift detected)

NATS JetStream provides persistence and replay, so downstream consumers (Snowflake ingestion, monitoring, alerting) can process events at their own pace.

Why NATS over Kafka: lower operational complexity, built-in request-reply pattern (useful for the ML Gateway), and sufficient throughput for most applications. If you genuinely need millions of events per second with multi-day retention across multiple DCs, Kafka is the right choice. Otherwise NATS is easier to run and reason about.

---

### 4. Analytical Plane — The Intelligence Engine

This is where Snowflake earns its place.

**Snowflake — Analytical Warehouse**

Data flows into Snowflake from three sources:

1. **PostgreSQL CDC** (via Fivetran, Airbyte, or Debezium → NATS → Snowpipe) — operational data replicated for analysis.
2. **Event stream** (NATS JetStream → cloud storage → Snowpipe) — user events, inference logs, system events.
3. **External data** (Snowflake Marketplace, partner feeds, third-party APIs) — enrichment data.

What lives in Snowflake:

- Complete inference history (every prediction, every input, every output) — this is your ML observability foundation.
- User behaviour data (event streams aggregated into analytical tables).
- Feature engineering pipelines (dbt models that compute training features from raw data).
- A/B experiment analysis (join experiment assignments with outcome events).
- Model performance dashboards (accuracy, drift, latency distributions over time).
- Business analytics (the usual: revenue, engagement, conversion, cohort analysis).

Snowflake virtual warehouses by workload:

- `WH_ETL` (Medium, auto-suspend 60s) — runs dbt transformations and data pipelines.
- `WH_BI` (Small, auto-suspend 120s, multi-cluster auto-scale) — serves dashboards and ad-hoc queries.
- `WH_ML` (Large, auto-suspend 60s) — runs feature computation and training data extraction.
- `WH_ADMIN` (X-Small, auto-suspend 300s) — runs monitoring queries and data quality checks.

**ML Training Pipeline**

Training happens offline, triggered by the orchestrator (Dagster or Airflow):

```
Snowflake (training data)
    → Export to cloud storage (Parquet)
        → Training job (GPU instance or SageMaker)
            → Model artifact → Model Registry (MLflow / Weights & Biases)
                → Config Service updates model version
                    → ML Serving hot-reloads new model
```

The training pipeline is never on the critical path. It runs asynchronously, on a schedule or triggered by drift detection. The Config UI shows training status, model versions, and allows the operator to promote a new model to production with a click.

**Feature Pipeline (Offline → Online)**

This is the closed loop that keeps the feature store fresh:

```
Raw events (Snowflake)
    → dbt feature models (SQL transformations)
        → Feature table in Snowflake
            → Export job → Redis (online feature store)
```

Cadence depends on the feature: some update hourly, some daily, some on every event. The config UI allows operators to adjust feature refresh schedules per feature group.

---

### 5. Configuration Plane — The Control Surface

This is what makes the system operable by non-engineers and is often the most overlooked part of ML systems.

**Model Registry**

Stores every trained model with:

- Version, training date, training data snapshot ID
- Evaluation metrics (accuracy, F1, AUC, etc.)
- Input/output schema
- Lineage (which features, which data, which hyperparameters)

The Config UI shows a list of model versions with their metrics. Promoting a model to production is a single action that updates the Config Service, which propagates via NATS to the ML Gateway.

**Feature Flags & Experiment Config**

Stored in PostgreSQL, edited via the Config UI:

```json
{
  "experiment_id": "rec_model_v3_rollout",
  "status": "active",
  "variants": [
    { "name": "control", "model_version": "rec_v2.4", "weight": 0.5 },
    { "name": "treatment", "model_version": "rec_v3.0", "weight": 0.5 }
  ],
  "targeting": {
    "user_segment": "premium",
    "geo": ["GB", "IE", "US"]
  },
  "success_metric": "conversion_rate_7d",
  "auto_stop": { "significance": 0.95, "min_sample": 10000 }
}
```

The ML Gateway reads this config (cached locally, refreshed on NATS events) and routes each request accordingly.

**Business Rules & Guardrails**

Not everything should be in the model. The Config UI also manages:

- Hard business rules (e.g., "never recommend out-of-stock items")
- Safety filters (e.g., "block predictions with confidence below X")
- Rate limits per customer tier
- Model fallback chains (if model A fails, try B, then serve cached)

These are applied as post-processing in the ML Gateway, after inference but before the response is returned.

---

## Request Lifecycle (End-to-End)

Here's what happens when a user hits the API or web app:

```
1. User request hits edge (Cloudflare)
   └─ Edge cache? → Return cached response (< 5ms)

2. Request reaches App Service (Axum)
   ├─ Auth check (JWT validation, < 1ms)
   ├─ Read user context from PostgreSQL (< 2ms, connection pooled)
   └─ Call ML Gateway (gRPC, internal network)

3. ML Gateway processes
   ├─ Read experiment config (local cache, < 0.1ms)
   ├─ Determine variant assignment (hash user_id, < 0.1ms)
   ├─ Fetch features from Redis (< 1ms)
   ├─ Call inference endpoint (gRPC)
   │   └─ Model inference (10-100ms depending on model)
   ├─ Apply business rules and threshold (< 0.1ms)
   ├─ Log to NATS (fire-and-forget, < 0.5ms)
   └─ Return result to App Service

4. App Service assembles response
   ├─ Merge ML result with application data
   └─ Return to user

5. Total latency: 20-150ms (depending on model complexity)

6. Asynchronously (after response):
   ├─ NATS delivers inference log to Snowflake (via Snowpipe)
   ├─ NATS delivers user event to analytics pipeline
   └─ Monitoring checks latency / error rate against thresholds
```

---

## Throughput Maximisation Strategies

### API Layer

- **Connection pooling** — pgBouncer in front of PostgreSQL, connection pool in Axum (deadpool or bb8).
- **Request coalescing** — If 100 users request the same recommendation list within 50ms, compute it once.
- **Async I/O everywhere** — Tokio runtime (Rust) or asyncio (Python); never block on I/O.
- **gRPC streaming** — For multi-result responses (e.g., "recommend 20 items"), stream results as they're ready rather than waiting for all 20.
- **Backpressure** — Use NATS queue groups and bounded channels so that no service overwhelms the next.

### ML Inference

- **Dynamic batching** — Triton/vLLM accumulate requests into GPU-efficient batches.
- **Model compilation** — TensorRT, ONNX Runtime, or torch.compile for optimised execution graphs.
- **Speculative execution** — Start inference before all features are ready if some features have predictable values.
- **Tiered models** — Lightweight model for p50 requests (fast path), heavy model only when lightweight model is uncertain.
- **Precomputation** — For stable entities (products, content), pre-compute predictions on a schedule and serve from Redis.

### Data Layer

- **Read replicas** — PostgreSQL read replicas for read-heavy application paths.
- **Materialised views** — Pre-compute common query patterns in PostgreSQL.
- **Hot/warm/cold tiering** — Recent data in PostgreSQL, historical in Snowflake, archived in cloud storage.
- **Partitioning** — Time-partition event tables in PostgreSQL; Snowflake handles this automatically.

---

## UX Maximisation Strategies

### For the End User

- **Streaming responses** — If ML inference takes > 200ms, stream partial results via SSE. Show a skeleton UI that fills in as results arrive.
- **Optimistic rendering** — Show predicted/cached results immediately, refine when fresh inference completes.
- **Graceful degradation** — If the ML system is slow or down, fall back to rule-based defaults rather than showing an error.
- **Progressive disclosure** — Don't wait for all ML results before rendering the page. Primary content first, ML-enhanced content streams in.
- **Perceived performance** — Prefetch likely next requests (e.g., if user is browsing category A, pre-warm recommendations for A).

### For the Config UI User (Operator)

- **Real-time feedback** — When an operator changes a threshold, show a live preview of how it would have affected the last N predictions.
- **Dry run mode** — "What if I deploy model v3?" Show side-by-side metrics comparison before committing.
- **One-click rollback** — Every config change is versioned. Rollback is a single button.
- **Guardrails on guardrails** — The Config UI itself has safety checks: "You're about to route 100% of traffic to an untested model. Are you sure?" with required confirmation.
- **Dashboards embedded in context** — Don't make operators switch to a separate monitoring tool. Show model performance metrics right next to the config controls.
- **Audit trail** — Every change is logged with who, what, when, and why (optional commit message).

---

## Technology Stack Summary

| Layer                  | Technology                 | Role                                         |
| ---------------------- | -------------------------- | -------------------------------------------- |
| Web frontend           | React                      | End-user UI, Config/Admin UI                 |
| API gateway            | Envoy/AWS                  | Routing, rate limiting, auth                 |
| App service            | Axum (Rust)                | Core business logic, high-throughput API     |
| ML-adjacent services   | FastAPI (Python)           | Services that need Python ML libraries       |
| ML Gateway             | Axum or FastAPI            | Model routing, feature assembly, A/B logic   |
| Internal comms         | gRPC (tonic in Rust)       | Service-to-service, type-safe, fast          |
| Event bus              | NATS JetStream             | Async events, config propagation, logging    |
| Operational DB         | PostgreSQL                 | ACID state, config, user data                |
| Feature store (online) | Redis / DragonflyDB        | Sub-ms feature lookup at inference time      |
| Cache                  | Redis + edge (Cloudflare)  | Response caching, result caching             |
| Analytical warehouse   | Snowflake                  | Historical analysis, feature engineering, BI |
| ML training            | PyTorch + cloud GPU        | Offline model training                       |
| ML serving             | Triton / vLLM / ort (Rust) | Online inference                             |
| Feature pipeline       | dbt + Dagster              | Snowflake → Redis feature materialisation    |
| CDC                    | Debezium or Fivetran       | PostgreSQL → Snowflake replication           |
| Model registry         | MLflow or W&B              | Model versioning, lineage, metrics           |
| Monitoring             | Prometheus + Grafana       | System metrics, model metrics, alerting      |

---

## What This Architecture Optimises For

1. **Latency** — The hot path (request → features → inference → response) is entirely in-memory or sub-millisecond stores. No analytical database on the critical path.
2. **Throughput** — Async I/O, dynamic batching, precomputation, and horizontal scaling at every layer.
3. **Operability** — The Config UI gives operators control without engineering deployments. Every change propagates in seconds via NATS.
4. **Observability** — Every inference is logged. Snowflake provides deep historical analysis. Monitoring catches drift and degradation.
5. **Separation of concerns** — PostgreSQL does what it's best at (transactions), Snowflake does what it's best at (analytics), Redis does what it's best at (fast reads), and the ML serving layer is independently scalable.
6. **Evolution** — Models can be swapped, features can be added, experiments can be run — all without touching application code. The configuration plane is the control surface that makes the system adaptable.

---

## Common Mistakes to Avoid

|Mistake|Why it hurts|What to do instead|
|---|---|---|
|Querying Snowflake in the hot request path|Adds 200ms+ to every request; not designed for point lookups|Pre-materialise to Redis; Snowflake is for batch/analytical only|
|Computing features at inference time from PostgreSQL|Slow, inconsistent, couples inference to operational DB load|Pre-compute features in Snowflake, materialise to Redis|
|Hardcoding model versions in application code|Every model change requires a deployment|Model registry + Config Service + hot reload|
|No fallback when ML is down|Users see errors when inference fails|Always have a deterministic fallback (cached, rule-based, or default)|
|Monolithic ML service that does routing, inference, and post-processing|Can't scale components independently; single point of failure|Separate ML Gateway (routing/logic) from ML Serving (inference)|
|Skipping the Config UI and using env vars or YAML for ML config|Operators can't self-serve; every change needs an engineer|Build the Config UI early; it pays for itself immediately|
|Logging inference results to PostgreSQL long-term|Table bloats, vacuuming pain, not designed for analytical queries|Log to NATS → Snowflake; keep only recent hot data in PostgreSQL|
|Training on stale data because the pipeline is manual|Model performance degrades silently|Automate with Dagster/Airflow; trigger retraining on drift detection|