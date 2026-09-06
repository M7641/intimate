# Cross-Service Distributed Tracing

Connecting traces across Python (FastAPI) and Rust (Axum) service boundaries so a single request produces one unified trace.

## End-to-End Scenario

```
Client                  red_db (Python)              tako (Rust)
  │                         │                           │
  │── POST /query ─────────▶│                           │
  │                         │── POST /upload ──────────▶│
  │                         │   traceparent: 00-abc...  │
  │                         │                           │── parse_multipart
  │                         │                           │── validate_schema
  │                         │                           │── s3_upload
  │                         │◀── 200 OK ────────────────│
  │◀── 200 OK ─────────────│                           │
  │                         │                           │
```

In Grafana Tempo, this appears as one trace:

```
[trace_id: abc123...]

red_db: POST /query                      ─── 250ms ─────────────────────┐
  ├── red_db: validate_request           ── 5ms ─┐                      │
  ├── red_db: httpx → POST tako/upload   ────── 200ms ────────────┐     │
  │     └── tako: POST /upload           ────── 190ms ───────────┐│     │
  │           ├── tako: parse_multipart  ── 20ms ─┐              ││     │
  │           ├── tako: validate_schema  ── 10ms ─┐              ││     │
  │           └── tako: s3_upload        ── 150ms ────────┐      ││     │
  └── red_db: format_response           ── 3ms ─┐               │      │
```

## How Propagation Works

### Step 1: Python Side — Automatic Injection

With `opentelemetry-instrumentation-httpx` installed and instrumented (see [03-fastapi.md](03-fastapi.md)), outgoing `httpx` calls automatically inject the `traceparent` header:

```python
import httpx

# This is all you need — the instrumentor handles the rest
async def call_tako(file_data: bytes) -> dict:
    async with httpx.AsyncClient() as client:
        response = await client.post(
            "http://localhost:3000/upload",
            files={"file": ("data.csv", file_data)},
        )
        return response.json()
```

Under the hood, the `httpx` instrumentor:
1. Reads the current span context (set by `FastAPIInstrumentor` for the incoming request)
2. Creates a `CLIENT` span for the outgoing call
3. Injects `traceparent` header: `00-{trace_id}-{new_span_id}-01`
4. The outgoing `traceparent` carries the same `trace_id` as the incoming request

### Step 2: Rust Side — Extraction

On the Rust side, the context propagation middleware (from [04-axum.md](04-axum.md)) extracts the incoming `traceparent`:

```rust
use opentelemetry::global;
use opentelemetry::propagation::Extractor;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tracing_opentelemetry::OpenTelemetrySpanExt;

// Set global propagator once at startup
global::set_text_map_propagator(TraceContextPropagator::new());

// Middleware that extracts context from incoming headers
async fn extract_otel_context(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(request.headers()))
    });

    // Link the current tracing span to the incoming OTel context
    tracing::Span::current().set_parent(parent_context);

    next.run(request).await
}

struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}
```

When `set_parent()` is called, the `OpenTelemetryLayer` sees the parent span context and uses the **same trace ID**. The Rust span becomes a child of the Python span.

## Rust → Python Direction

If a Rust service needs to call a Python service:

```rust
use reqwest::Client;
use opentelemetry::global;
use opentelemetry::propagation::Injector;
use tracing_opentelemetry::OpenTelemetrySpanExt;

async fn call_red_db(client: &Client, query: &str) -> Result<String, reqwest::Error> {
    let mut headers = reqwest::header::HeaderMap::new();

    // Inject current trace context into outgoing headers
    let context = tracing::Span::current().context();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&context, &mut HeaderMapInjector(&mut headers));
    });

    let response = client
        .post("http://localhost:8000/query")
        .headers(headers)
        .json(&serde_json::json!({"query": query}))
        .send()
        .await?;

    response.text().await
}

struct HeaderMapInjector<'a>(&'a mut reqwest::header::HeaderMap);

impl Injector for HeaderMapInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(&value) {
                self.0.insert(name, val);
            }
        }
    }
}
```

## Baggage Propagation

Baggage lets you pass key-value pairs across service boundaries — useful for cross-cutting data like `user_id` or `tenant_id` without adding them as span attributes at every hop.

### Python Side (Injecting Baggage)

```python
from opentelemetry import baggage, context
from opentelemetry.propagate import set_global_textmap
from opentelemetry.propagators.composite import CompositePropagator
from opentelemetry.baggage.propagation import W3CBaggagePropagator
from opentelemetry.trace.propagation import TraceContextTextMapPropagator

# Set composite propagator (TraceContext + Baggage) at startup:
set_global_textmap(CompositePropagator([
    TraceContextTextMapPropagator(),
    W3CBaggagePropagator(),
]))

# In a request handler:
ctx = baggage.set_baggage("user_id", "u-12345")
context.attach(ctx)
# Subsequent outgoing httpx calls will include both traceparent and baggage headers
```

### Rust Side (Reading Baggage)

```rust
use opentelemetry::baggage::BaggageExt;

async fn handler(request: Request) -> Response {
    let cx = opentelemetry::Context::current();
    if let Some(user_id) = cx.baggage().get("user_id") {
        tracing::info!(user_id = %user_id, "processing request for user");
    }
    // ...
}
```

## Debugging: Verifying Trace Continuity

If traces aren't connecting across services, check these common issues:

### 1. Verify Headers are Present

```bash
# On the Python side, log outgoing headers:
curl -v http://localhost:3000/upload 2>&1 | grep traceparent
# Should see: traceparent: 00-<32hex>-<16hex>-01
```

### 2. Check Collector Logs

Enable the `debug` exporter in the collector config (already in [02-collector-setup.md](02-collector-setup.md)):

```yaml
exporters:
  debug:
    verbosity: detailed    # Change from "basic" to "detailed"
```

Then check logs:
```bash
docker compose logs otel-collector | grep trace_id
# Verify the same trace_id appears for both red_db and tako spans
```

### 3. Common Failures

| Symptom | Cause | Fix |
|---------|-------|-----|
| Two separate traces instead of one | `traceparent` not being injected | Verify `HTTPXClientInstrumentor().instrument()` was called |
| Rust spans have no parent | Context not extracted | Verify `extract_otel_context` middleware is layered correctly |
| `traceparent` header present but Rust creates new trace | Propagator not set | Call `global::set_text_map_propagator(TraceContextPropagator::new())` at startup |
| Baggage not arriving | Only `TraceContextPropagator` set | Use `CompositePropagator` with `W3CBaggagePropagator` |

### 4. End-to-End Verification Test

```python
# test_e2e_trace.py — run with collector stack up
import httpx
import asyncio

async def test_cross_service_trace():
    """Verify a request through red_db → tako produces a single trace."""
    async with httpx.AsyncClient() as client:
        # Hit red_db which internally calls tako
        response = await client.post(
            "http://localhost:8000/query",
            json={"query": "SELECT 1"},
        )
        assert response.status_code == 200

    # Wait for spans to be exported (batch processor delay)
    await asyncio.sleep(6)

    # Query Tempo for traces from red_db
    async with httpx.AsyncClient() as client:
        response = await client.get(
            "http://localhost:3200/api/search",
            params={"tags": "service.name=red_db", "limit": 1},
        )
        traces = response.json()["traces"]
        assert len(traces) > 0

        # Fetch the full trace
        trace_id = traces[0]["traceID"]
        response = await client.get(f"http://localhost:3200/api/traces/{trace_id}")
        trace = response.json()

        # Verify spans from both services
        service_names = {
            span["process"]["serviceName"]
            for batch in trace["batches"]
            for span in batch["spans"]
        }
        assert "red_db" in service_names, "Missing red_db spans"
        assert "tako" in service_names, "Missing tako spans"
        print(f"Trace {trace_id} spans both red_db and tako")

asyncio.run(test_cross_service_trace())
```

## Next Steps

- [06-production.md](06-production.md) — sampling, performance tuning, security
- [07-migration.md](07-migration.md) — phased rollout plan
