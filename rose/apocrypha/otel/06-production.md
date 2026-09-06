# Production Considerations

Tuning OpenTelemetry for performance, security, and cost in production.

## Sampling

Not every trace needs to be recorded. Sampling controls what percentage of traces are captured.

### Head-Based Sampling

Decides at the **start** of a trace whether to record it. The sampled/not-sampled decision propagates to all child spans via the `traceparent` flags field.

**Recommended: `ParentBasedTraceIdRatio`**

```
Parent decides → if no parent, use TraceIdRatio
                 if parent is sampled, sample this too
                 if parent is not sampled, don't sample
```

This ensures all spans in a trace agree on whether to sample, avoiding orphaned child spans.

#### Python Configuration

```bash
OTEL_TRACES_SAMPLER=parentbased_traceidratio
OTEL_TRACES_SAMPLER_ARG=0.1    # 10% of traces in production
```

Or programmatically:

```python
from opentelemetry.sdk.trace.sampling import ParentBasedTraceIdRatio

tracer_provider = TracerProvider(
    resource=resource,
    sampler=ParentBasedTraceIdRatio(0.1),  # 10%
)
```

#### Rust Configuration

```bash
OTEL_TRACES_SAMPLER=parentbased_traceidratio
OTEL_TRACES_SAMPLER_ARG=0.1
```

Or programmatically:

```rust
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};

let provider = SdkTracerProvider::builder()
    .with_sampler(Sampler::ParentBased(Box::new(
        Sampler::TraceIdRatioBased(0.1),
    )))
    .with_batch_exporter(exporter)
    .with_resource(resource)
    .build();
```

### Recommended Rates

| Environment | Rate | Rationale |
|------------|------|-----------|
| Local dev | `1.0` (100%) | See every trace for debugging |
| Staging | `1.0` (100%) | Full visibility for pre-prod validation |
| Production | `0.1` (10%) | Balance between visibility and cost |
| Production (high traffic) | `0.01` (1%) | If volume exceeds budget |

### Tail-Based Sampling (Collector)

The Collector can make sampling decisions **after** seeing all spans in a trace. This is powerful for keeping error traces at 100% while sampling successful ones at 10%.

Add to `otel-collector-config.yaml`:

```yaml
processors:
  tail_sampling:
    decision_wait: 10s          # Wait this long for all spans to arrive
    num_traces: 100000          # Max traces held in memory
    policies:
      # Always keep error traces
      - name: errors
        type: status_code
        status_code:
          status_codes: [ERROR]
      # Always keep slow traces (>2s)
      - name: slow
        type: latency
        latency:
          threshold_ms: 2000
      # Sample everything else at 10%
      - name: default
        type: probabilistic
        probabilistic:
          sampling_percentage: 10

service:
  pipelines:
    traces:
      processors: [tail_sampling, batch, resource]   # tail_sampling BEFORE batch
```

**Trade-off:** Tail sampling requires the Collector to buffer traces in memory. The `decision_wait` must be long enough for all spans to arrive but short enough to avoid memory pressure.

## Performance

### BatchSpanProcessor — Always

Never use `SimpleSpanProcessor` in production. It exports synchronously on every span close, blocking your request handler.

`BatchSpanProcessor` (the default in both Python and Rust SDKs) buffers spans and exports them in batches on a background thread/task.

### Tuning Parameters

| Parameter | Default | Recommended | Notes |
|-----------|---------|-------------|-------|
| `max_queue_size` | 2048 | 4096 | Max spans buffered. If full, new spans are dropped |
| `scheduled_delay` | 5000ms | 5000ms | How often the batch is exported |
| `max_export_batch_size` | 512 | 512 | Max spans per export call |
| `export_timeout` | 30000ms | 10000ms | Timeout per export. Lower to fail fast |

#### Python

```python
from opentelemetry.sdk.trace.export import BatchSpanProcessor

processor = BatchSpanProcessor(
    OTLPSpanExporter(),
    max_queue_size=4096,
    schedule_delay_millis=5000,
    max_export_batch_size=512,
    export_timeout_millis=10000,
)
```

#### Rust

```rust
use opentelemetry_sdk::trace::BatchConfigBuilder;

let provider = SdkTracerProvider::builder()
    .with_batch_exporter(exporter)
    // Batch config is set via environment variables:
    // OTEL_BSP_MAX_QUEUE_SIZE=4096
    // OTEL_BSP_SCHEDULE_DELAY=5000
    // OTEL_BSP_MAX_EXPORT_BATCH_SIZE=512
    // OTEL_BSP_EXPORT_TIMEOUT=10000
    .build();
```

### Expected Overhead

With `BatchSpanProcessor` and 10% sampling:
- **CPU:** <1% additional overhead
- **Memory:** ~2-4 MB for the span queue
- **Network:** Minimal — batched, compressed gRPC
- **Latency:** Zero impact on request handling (export is async)

## Security

### Never Include PII in Span Attributes

Spans are stored in Tempo and visible to anyone with Grafana access. Never add:
- Passwords, tokens, API keys
- Email addresses, names, phone numbers
- Full SQL queries with user data (truncate or parameterize)
- Request/response bodies

```python
# BAD:
span.set_attribute("user.email", user.email)
span.set_attribute("db.statement", f"SELECT * FROM users WHERE email = '{email}'")

# GOOD:
span.set_attribute("user.id", user.id)
span.set_attribute("db.statement", "SELECT * FROM users WHERE email = ?")
span.set_attribute("db.operation", "SELECT")
```

### SQL Query Redaction

For Redshift/Snowflake queries, truncate the statement and strip parameters:

```python
import re

def redact_sql(query: str, max_length: int = 200) -> str:
    # Replace string literals with '?'
    redacted = re.sub(r"'[^']*'", "?", query)
    return redacted[:max_length]

span.set_attribute("db.statement", redact_sql(query))
```

### Collector Attribute Allow-List

The Collector's `attributes` processor can strip sensitive fields from all telemetry before it reaches backends:

```yaml
processors:
  attributes:
    actions:
      # Remove any accidentally-included sensitive attributes
      - key: user.email
        action: delete
      - key: http.request.body
        action: delete
      - key: http.response.body
        action: delete
```

## Resource Attributes

Every service should set these standard resource attributes:

| Attribute | Example | Source |
|-----------|---------|--------|
| `service.name` | `"flow"` | `OTEL_SERVICE_NAME` env var or SDK code |
| `service.version` | `"0.1.0"` | Read from `Cargo.toml` / `pyproject.toml` |
| `deployment.environment` | `"production"` | `OTEL_RESOURCE_ATTRIBUTES` or Collector `resource` processor |

```bash
# Set via environment (works for both Python and Rust):
OTEL_SERVICE_NAME=flow
OTEL_RESOURCE_ATTRIBUTES=service.version=0.1.0,deployment.environment=production
```

The Collector's `resource` processor (in [02-collector-setup.md](02-collector-setup.md)) adds `deployment.environment` automatically, so services only need to set their own `service.name` and `service.version`.

## Cost Control

### Filter Noisy Spans

The Collector's `filter` processor (already configured) drops `/health` check spans. Extend it for other noisy endpoints:

```yaml
processors:
  filter:
    error_mode: ignore
    traces:
      span:
        - 'attributes["http.route"] == "/health"'
        - 'attributes["http.route"] == "/readiness"'
        - 'attributes["http.route"] == "/prometheus/metrics"'
```

### Limit Attribute Cardinality

High-cardinality attributes (unique user IDs, request IDs) cause storage bloat in Tempo. Use them sparingly:

```python
# BAD: unique per request → infinite cardinality
span.set_attribute("request.id", str(uuid4()))

# GOOD: bounded cardinality
span.set_attribute("request.method", "POST")
span.set_attribute("request.route", "/streams/{stream_id}/events")
```

### Storage Retention

Tempo's local storage retention is set in `tempo-config.yaml`:

```yaml
compactor:
  compaction:
    block_retention: 72h    # Local dev: 3 days
    # Production: 168h (7 days) or more, depending on volume
```

## Next Steps

- [07-migration.md](07-migration.md) — phased rollout plan for the monorepo
