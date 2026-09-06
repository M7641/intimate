# FastAPI Instrumentation

Adding OpenTelemetry traces, metrics, and log correlation to Python/FastAPI services.

**Primary example:** `cocoon/red_db` (already has Prometheus — most complex case).
**Also applies to:** `cocoon/snow_db`, `blank/app_one`, `blank/app_two`, `blank/react_minimal`.

## Dependencies

Add to `pyproject.toml`:

```toml
[project]
dependencies = [
    # ... existing deps ...
    "opentelemetry-api>=1.24.0",
    "opentelemetry-sdk>=1.24.0",
    "opentelemetry-exporter-otlp>=1.24.0",
    "opentelemetry-instrumentation-fastapi>=0.45b0",
    "opentelemetry-instrumentation-logging>=0.45b0",
    "opentelemetry-instrumentation-httpx>=0.45b0",     # if using httpx for outgoing calls
]
```

## Setup Function

Create a setup function called once at startup. This configures the TracerProvider, MeterProvider, and log correlation:

```python
# src/red_db/otel.py

from opentelemetry import trace, metrics
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from opentelemetry.exporter.otlp.proto.http.metric_exporter import OTLPMetricExporter
from opentelemetry.instrumentation.fastapi import FastAPIInstrumentor
from opentelemetry.instrumentation.logging import LoggingInstrumentor
from opentelemetry.sdk.metrics import MeterProvider
from opentelemetry.sdk.metrics.export import PeriodicExportingMetricReader
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor


def setup_otel(app, *, service_name: str = "red_db") -> None:
    """
    Initialize OpenTelemetry for a FastAPI application.

    Call this in the FastAPI lifespan, before the app starts serving.
    Reads OTEL_EXPORTER_OTLP_ENDPOINT from env (default: http://localhost:4318).
    """
    resource = Resource.create(
        {
            "service.name": service_name,
            "service.version": "0.1.0",
        }
    )

    # ── Traces ──────────────────────────────────────────────
    tracer_provider = TracerProvider(resource=resource)
    tracer_provider.add_span_processor(
        BatchSpanProcessor(OTLPSpanExporter())
        # OTLPSpanExporter reads OTEL_EXPORTER_OTLP_ENDPOINT from env
    )
    trace.set_tracer_provider(tracer_provider)

    # ── Metrics ─────────────────────────────────────────────
    metric_reader = PeriodicExportingMetricReader(
        OTLPMetricExporter(),
        export_interval_millis=15_000,
    )
    meter_provider = MeterProvider(resource=resource, metric_readers=[metric_reader])
    metrics.set_meter_provider(meter_provider)

    # ── Auto-instrumentation ────────────────────────────────
    FastAPIInstrumentor().instrument_app(app)
    LoggingInstrumentor().instrument(set_logging_format=True)

    # ── Shutdown hook ───────────────────────────────────────
    import atexit
    atexit.register(tracer_provider.shutdown)
    atexit.register(meter_provider.shutdown)
```

## Integration with FastAPI Lifespan

In `red_db`, the app is created in `api.py`. Add the OTel setup to the existing lifespan:

```python
# cocoon/red_db/src/red_db/api/api.py

from contextlib import asynccontextmanager
from red_db.otel import setup_otel

@asynccontextmanager
async def lifespan(app: FastAPI):
    # Existing startup logic stays here
    setup_otel(app, service_name="red_db")
    yield
    # Existing shutdown logic stays here

app = FastAPI(lifespan=lifespan)

# Existing Prometheus instrumentation — KEEP this, OTel runs alongside it
Instrumentator().instrument(app).expose(app, endpoint="/prometheus/metrics")
```

### What `FastAPIInstrumentor` Does Automatically

Once instrumented, every incoming request gets:

- A `SERVER` span with: `http.method`, `http.route`, `http.status_code`, `http.url`, `net.host.port`
- Automatic context extraction from incoming `traceparent` headers
- Automatic context injection into responses
- Exception recording on 5xx responses

No changes to route handlers needed.

## Log Correlation with NimbusLogger

`NimbusLogger` wraps Python's stdlib `logging`. The `LoggingInstrumentor` monkey-patches the `logging` module to inject `trace_id`, `span_id`, and `service.name` into every log record's attributes.

To make these visible in NimbusLogger output, update the format string in `blank/pure/src/pure/logging.py`:

```python
# Before:
format_string = (
    "%(asctime)s - %(name)s - %(funcName)20s() - %(levelname)s - %(message)s "
    "(%(filename)s:%(lineno)d)"
)

# After (adds trace context):
format_string = (
    "%(asctime)s - %(name)s - %(funcName)20s() - %(levelname)s - "
    "[trace_id=%(otelTraceID)s span_id=%(otelSpanID)s] - %(message)s "
    "(%(filename)s:%(lineno)d)"
)
```

Now every NimbusLogger line includes the trace ID. You can copy this ID from a log line and search for it in Grafana Tempo to see the full trace.

**Important:** If OTel is not initialized (e.g., in tests or services that haven't adopted OTel yet), these fields default to `"0"`, so existing logging won't break.

## Manual Spans for Business Logic

Auto-instrumentation covers HTTP boundaries. For internal operations (database queries, S3 uploads, etc.), create manual spans:

```python
from opentelemetry import trace

tracer = trace.get_tracer(__name__)

async def execute_redshift_query(query: str, params: dict):
    with tracer.start_as_current_span(
        "redshift.query",
        attributes={
            "db.system": "redshift",
            "db.statement": query[:200],    # Truncate to avoid PII leaks
            "db.operation": "SELECT",
        },
    ) as span:
        try:
            result = await run_query(query, params)
            span.set_attribute("db.row_count", len(result))
            return result
        except Exception as e:
            span.set_status(trace.StatusCode.ERROR, str(e))
            span.record_exception(e)
            raise
```

Similarly for Snowflake queries in `snow_db`:

```python
async def execute_snowflake_query(query: str):
    with tracer.start_as_current_span(
        "snowflake.query",
        attributes={
            "db.system": "snowflake",
            "db.statement": query[:200],
        },
    ) as span:
        result = await run_sf_query(query)
        span.set_attribute("db.row_count", len(result))
        return result
```

## Outgoing HTTP Calls

If `red_db` calls `tako` (or any other service) via `httpx`, instrument outgoing calls to propagate trace context:

```python
# In pyproject.toml:
# "opentelemetry-instrumentation-httpx>=0.45b0"

from opentelemetry.instrumentation.httpx import HTTPXClientInstrumentor

# Call once in setup_otel():
HTTPXClientInstrumentor().instrument()
```

This automatically injects `traceparent` headers into all outgoing `httpx` requests. No code changes to the call sites.

## Configuration via Environment Variables

OTel SDKs read standard env vars — no hardcoded endpoints:

```bash
# .env or docker-compose environment:
OTEL_SERVICE_NAME=red_db
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318    # HTTP for Python
OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
OTEL_TRACES_SAMPLER=parentbased_traceidratio
OTEL_TRACES_SAMPLER_ARG=1.0                           # 100% in dev, 0.1 in prod
OTEL_RESOURCE_ATTRIBUTES=deployment.environment=local
```

If `OTEL_EXPORTER_OTLP_ENDPOINT` is not set, the SDK defaults to `http://localhost:4318` — which matches the [collector setup](02-collector-setup.md).

## Service-Specific Notes

### `red_db`

- Already has `prometheus-fastapi-instrumentator` — keep it. OTel metrics run alongside
- Has `log_threads_in_use_middleware` — this will automatically get a parent span from `FastAPIInstrumentor`
- Consider adding manual spans for Redshift query execution

### `snow_db`

- Same pattern as `red_db` but without Prometheus — simpler setup
- Add manual spans for Snowflake queries (these are often the slowest operations)

### `app_one`, `app_two`, `react_minimal`

- Apply the same `setup_otel()` pattern
- Change `service_name` parameter for each

## Verification

After setup, verify traces are flowing:

```bash
# 1. Start the collector stack
cd apocrypha/otel && docker compose up -d

# 2. Start red_db with OTel env vars
OTEL_SERVICE_NAME=red_db OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 \
  uvicorn red_db.api.api:app --reload

# 3. Make a request
curl http://localhost:8000/health

# 4. Check collector logs for received spans
docker compose logs otel-collector | grep "TracesExporter"

# 5. Open Grafana Tempo
open http://localhost:3001/explore
# Select Tempo → Search → service.name = "red_db"
```

## Next Steps

- [04-axum.md](04-axum.md) — instrument the Rust services
- [05-cross-service.md](05-cross-service.md) — connect Python → Rust traces
