"""Spawner — observability demo API.

Each endpoint group generates a different type of telemetry data so you can
see it flowing through the OTel Collector into Tempo, Loki, Prometheus, and
(optionally) Pyroscope.

Run:  uv run spawner run
Then: curl http://localhost:8000/
"""

import asyncio
import logging
import random
import time
import uuid
from contextlib import asynccontextmanager

from fastapi import FastAPI, HTTPException
from opentelemetry import metrics, trace

from spawner.otel import setup_otel, shutdown_otel

logger = logging.getLogger("spawner")

# ---------------------------------------------------------------------------
# Globals populated at startup
# ---------------------------------------------------------------------------
_providers = None
tracer = trace.get_tracer("spawner")
meter = metrics.get_meter("spawner")


# --- Metrics instruments (created once, used by endpoints) -----------------
request_counter = None
order_counter = None
order_value_histogram = None
workflow_active_gauge = None
workflow_queue_depth = None
workflow_step_duration = None
workflow_last_transition = None

# In-memory workflow store
workflows: dict[str, dict] = {}


def _init_meters():
    global request_counter, order_counter, order_value_histogram
    global workflow_active_gauge, workflow_queue_depth
    global workflow_step_duration, workflow_last_transition

    request_counter = meter.create_counter(
        "spawner.requests",
        description="Total demo requests",
        unit="1",
    )
    order_counter = meter.create_counter(
        "spawner.orders",
        description="Total simulated orders",
        unit="1",
    )
    order_value_histogram = meter.create_histogram(
        "spawner.order.value",
        description="Simulated order values",
        unit="USD",
    )
    workflow_active_gauge = meter.create_up_down_counter(
        "workflow.active_count",
        description="Currently active workflows",
        unit="1",
    )
    workflow_queue_depth = meter.create_up_down_counter(
        "workflow.queue_depth",
        description="Workflows waiting to be processed",
        unit="1",
    )
    workflow_step_duration = meter.create_histogram(
        "workflow.step.duration",
        description="Duration of each workflow step",
        unit="s",
    )


@asynccontextmanager
async def lifespan(app: FastAPI):
    global _providers
    _providers = setup_otel(app)
    _init_meters()
    logger.info("spawner started — OTel pipelines active")
    yield
    logger.info("spawner shutting down — flushing telemetry")
    shutdown_otel(*_providers)


app = FastAPI(
    title="Spawner",
    description="Observability demo — traces, metrics, logs & profiling",
    version="0.1.0",
    lifespan=lifespan,
)


# ===========================================================================
# Health
# ===========================================================================


@app.get("/health")
async def health():
    return {"status": "ok"}


@app.get("/")
async def index():
    return {
        "service": "spawner",
        "signals": {
            "traces": [
                "GET /traces/simple",
                "GET /traces/nested",
                "GET /traces/error",
            ],
            "metrics": [
                "POST /metrics/order",
                "GET  /metrics/status",
            ],
            "logs": [
                "GET /logs/levels",
                "GET /logs/structured",
            ],
            "workflows": [
                "POST /workflows/",
                "POST /workflows/{id}/step",
                "GET  /workflows/{id}",
                "GET  /workflows/",
            ],
            "profiling": [
                "GET /profile/fibonacci/{n}",
                "GET /profile/sort/{size}",
            ],
        },
    }


# ===========================================================================
# TRACES — demonstrate span creation, nesting, and error recording
# ===========================================================================


@app.get("/traces/simple")
async def trace_simple():
    """Creates a single custom span with attributes."""
    with tracer.start_as_current_span(
        "simple-operation",
        attributes={"demo.type": "simple", "demo.source": "spawner"},
    ) as span:
        await asyncio.sleep(random.uniform(0.01, 0.05))
        span.set_attribute("demo.result", "success")
        return {"trace_id": format(span.get_span_context().trace_id, "032x")}


@app.get("/traces/nested")
async def trace_nested():
    """Creates a 3-level span hierarchy: ingest → validate → persist."""
    with tracer.start_as_current_span("ingest", attributes={"step": 1}) as root:
        await asyncio.sleep(random.uniform(0.01, 0.03))

        with tracer.start_as_current_span("validate", attributes={"step": 2}):
            await asyncio.sleep(random.uniform(0.02, 0.06))
            logger.info("validation passed", extra={"record_count": 42})

        with tracer.start_as_current_span("persist", attributes={"step": 3}):
            await asyncio.sleep(random.uniform(0.03, 0.08))
            logger.info("persisted to store", extra={"rows_written": 42})

        return {
            "trace_id": format(root.get_span_context().trace_id, "032x"),
            "spans": ["ingest", "validate", "persist"],
        }


@app.get("/traces/error")
async def trace_error():
    """Creates a span that records an exception — visible in Tempo as an error span."""
    with tracer.start_as_current_span("failing-operation") as span:
        try:
            await asyncio.sleep(0.02)
            msg = "simulated failure for observability demo"
            raise ValueError(msg)
        except ValueError as exc:
            span.set_status(trace.StatusCode.ERROR, str(exc))
            span.record_exception(exc)
            return {
                "trace_id": format(span.get_span_context().trace_id, "032x"),
                "error": str(exc),
            }


# ===========================================================================
# METRICS — demonstrate counters, histograms, gauges
# ===========================================================================


@app.post("/metrics/order")
async def metrics_order():
    """Simulates an order: increments counter + records value in histogram."""
    value = round(random.uniform(5.0, 500.0), 2)
    status = random.choice(["completed", "pending", "cancelled"])

    order_counter.add(1, {"order.status": status})
    order_value_histogram.record(value, {"order.status": status})
    request_counter.add(1, {"endpoint": "/metrics/order"})

    logger.info("order placed", extra={"order_value": value, "order_status": status})
    return {"order_value": value, "status": status}


@app.get("/metrics/status")
async def metrics_status():
    """Returns a summary of what metrics are being emitted."""
    request_counter.add(1, {"endpoint": "/metrics/status"})
    return {
        "instruments": [
            {"name": "spawner.requests", "type": "counter"},
            {"name": "spawner.orders", "type": "counter"},
            {"name": "spawner.order.value", "type": "histogram", "unit": "USD"},
            {"name": "workflow.active_count", "type": "up_down_counter"},
            {"name": "workflow.queue_depth", "type": "up_down_counter"},
            {"name": "workflow.step.duration", "type": "histogram", "unit": "s"},
        ],
        "note": "Exported to Prometheus via OTel Collector every 5s",
    }


# ===========================================================================
# LOGS — demonstrate log levels and structured logging with trace correlation
# ===========================================================================


@app.get("/logs/levels")
async def logs_levels():
    """Emits one log at each level — all correlated with the current trace."""
    with tracer.start_as_current_span("log-demo") as span:
        logger.debug("this is a DEBUG log")
        logger.info("this is an INFO log")
        logger.warning("this is a WARNING log")
        logger.error("this is an ERROR log")
        return {
            "trace_id": format(span.get_span_context().trace_id, "032x"),
            "logs_emitted": ["DEBUG", "INFO", "WARNING", "ERROR"],
            "note": "Each log record includes trace_id and span_id — check Loki",
        }


@app.get("/logs/structured")
async def logs_structured():
    """Emits structured log with custom fields — shows up as labels in Loki."""
    with tracer.start_as_current_span("structured-log-demo") as span:
        logger.info(
            "user action completed",
            extra={
                "user_id": "usr_" + uuid.uuid4().hex[:8],
                "action": "data_export",
                "duration_ms": random.randint(50, 2000),
                "records_processed": random.randint(100, 10000),
            },
        )
        return {
            "trace_id": format(span.get_span_context().trace_id, "032x"),
            "note": "Check Loki for structured fields in log record",
        }


# ===========================================================================
# WORKFLOWS — simulate multi-step, long-running event-sourced workflows
# with custom metrics matching the Grafana workflows dashboard
# ===========================================================================

WORKFLOW_STEPS = [
    "queued",
    "validating",
    "processing",
    "enriching",
    "finalizing",
    "completed",
]


@app.post("/workflows/")
async def workflow_start(workflow_type: str = "data_pipeline"):
    """Start a new workflow — enters 'queued' state."""
    wf_id = "wf_" + uuid.uuid4().hex[:8]
    workflows[wf_id] = {
        "id": wf_id,
        "type": workflow_type,
        "step": "queued",
        "step_index": 0,
        "started_at": time.time(),
        "last_transition": time.time(),
        "outcome": None,
    }

    workflow_active_gauge.add(1, {"workflow.type": workflow_type})
    workflow_queue_depth.add(1, {"workflow.type": workflow_type})

    with tracer.start_as_current_span(
        "workflow.start",
        attributes={
            "workflow.id": wf_id,
            "workflow.type": workflow_type,
            "workflow.step": "queued",
        },
    ) as span:
        logger.info(
            "workflow started", extra={"workflow_id": wf_id, "type": workflow_type}
        )
        return {
            "trace_id": format(span.get_span_context().trace_id, "032x"),
            **workflows[wf_id],
        }


@app.post("/workflows/{wf_id}/step")
async def workflow_advance(wf_id: str):
    """Advance the workflow by one step — records step duration and transitions."""
    if wf_id not in workflows:
        raise HTTPException(status_code=404, detail=f"Workflow {wf_id} not found")

    wf = workflows[wf_id]
    if wf["outcome"] is not None:
        raise HTTPException(status_code=400, detail=f"Workflow already {wf['outcome']}")

    prev_step = wf["step"]

    # Simulate work
    duration = random.uniform(0.05, 0.3)
    await asyncio.sleep(duration)

    # Possible random failure
    if random.random() < 0.1:
        wf["outcome"] = "failure"
        workflow_active_gauge.add(-1, {"workflow.type": wf["type"]})
        if prev_step == "queued":
            workflow_queue_depth.add(-1, {"workflow.type": wf["type"]})

        with tracer.start_as_current_span(
            "workflow.step",
            attributes={
                "workflow.id": wf_id,
                "workflow.type": wf["type"],
                "workflow.step": prev_step,
                "workflow.outcome": "failure",
            },
        ) as span:
            span.set_status(trace.StatusCode.ERROR, "step failed")
            workflow_step_duration.record(
                duration,
                {
                    "workflow.type": wf["type"],
                    "workflow.step": prev_step,
                    "workflow.outcome": "failure",
                },
            )
            logger.error(
                "workflow step failed", extra={"workflow_id": wf_id, "step": prev_step}
            )
            return {
                "trace_id": format(span.get_span_context().trace_id, "032x"),
                **wf,
            }

    # Advance to next step
    next_index = wf["step_index"] + 1
    if next_index >= len(WORKFLOW_STEPS):
        next_index = len(WORKFLOW_STEPS) - 1

    next_step = WORKFLOW_STEPS[next_index]
    wf["step"] = next_step
    wf["step_index"] = next_index
    wf["last_transition"] = time.time()

    if prev_step == "queued":
        workflow_queue_depth.add(-1, {"workflow.type": wf["type"]})

    if next_step == "completed":
        wf["outcome"] = "success"
        workflow_active_gauge.add(-1, {"workflow.type": wf["type"]})

    with tracer.start_as_current_span(
        "workflow.step",
        attributes={
            "workflow.id": wf_id,
            "workflow.type": wf["type"],
            "workflow.step": next_step,
            "workflow.outcome": wf["outcome"] or "in_progress",
        },
    ) as span:
        workflow_step_duration.record(
            duration,
            {
                "workflow.type": wf["type"],
                "workflow.step": next_step,
                "workflow.outcome": wf["outcome"] or "in_progress",
            },
        )
        logger.info(
            "workflow step completed",
            extra={
                "workflow_id": wf_id,
                "from": prev_step,
                "to": next_step,
                "duration_s": round(duration, 3),
            },
        )
        return {
            "trace_id": format(span.get_span_context().trace_id, "032x"),
            **wf,
        }


@app.get("/workflows/{wf_id}")
async def workflow_status(wf_id: str):
    """Get current status of a workflow."""
    if wf_id not in workflows:
        raise HTTPException(status_code=404, detail=f"Workflow {wf_id} not found")
    wf = workflows[wf_id]
    stalled_seconds = time.time() - wf["last_transition"]
    return {**wf, "stalled_seconds": round(stalled_seconds, 1)}


@app.get("/workflows/")
async def workflow_list():
    """List all workflows with stalled detection."""
    now = time.time()
    result = []
    for wf in workflows.values():
        stalled = now - wf["last_transition"]
        result.append({**wf, "stalled_seconds": round(stalled, 1)})
    return {"workflows": result, "total": len(result)}


# ===========================================================================
# PROFILING — CPU-intensive endpoints for Pyroscope flamegraphs
# ===========================================================================


@app.get("/profile/fibonacci/{n}")
async def profile_fibonacci(n: int):
    """Recursive fibonacci — deliberately slow for profiling. Keep n < 35."""
    if n > 35:
        raise HTTPException(status_code=400, detail="n must be <= 35 to avoid timeout")

    with tracer.start_as_current_span(
        "fibonacci", attributes={"fibonacci.n": n}
    ) as span:
        start = time.monotonic()
        result = _fib(n)
        elapsed = time.monotonic() - start
        span.set_attribute("fibonacci.result", result)
        span.set_attribute("fibonacci.duration_s", round(elapsed, 4))
        logger.info(
            "fibonacci computed",
            extra={"n": n, "result": result, "duration_s": round(elapsed, 4)},
        )
        return {"n": n, "result": result, "duration_s": round(elapsed, 4)}


def _fib(n: int) -> int:
    if n < 2:
        return n
    return _fib(n - 1) + _fib(n - 2)


@app.get("/profile/sort/{size}")
async def profile_sort(size: int):
    """Generate and sort a random list — shows up in CPU profiles."""
    if size > 5_000_000:
        raise HTTPException(status_code=400, detail="size must be <= 5000000")

    with tracer.start_as_current_span("sort", attributes={"sort.size": size}) as span:
        data = [random.random() for _ in range(size)]
        start = time.monotonic()
        data.sort()
        elapsed = time.monotonic() - start
        span.set_attribute("sort.duration_s", round(elapsed, 4))
        logger.info(
            "sort completed", extra={"size": size, "duration_s": round(elapsed, 4)}
        )
        return {"size": size, "duration_s": round(elapsed, 4)}
