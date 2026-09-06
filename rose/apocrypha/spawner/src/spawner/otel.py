"""OpenTelemetry setup — traces, metrics, and logs via OTLP HTTP.

Configures all three signal pipelines to export to the OTel Collector.
When the collector is not running, the app still works — exporters fail
silently and endpoints respond normally.

Environment variables (standard OTel):
    OTEL_SERVICE_NAME          — defaults to "spawner"
    OTEL_EXPORTER_OTLP_ENDPOINT — defaults to "http://localhost:4318"
"""

import logging
import os

from opentelemetry import metrics, trace
from opentelemetry.exporter.otlp.proto.http.metric_exporter import (
    OTLPMetricExporter,
)
from opentelemetry.exporter.otlp.proto.http.trace_exporter import (
    OTLPSpanExporter,
)
from opentelemetry.instrumentation.fastapi import FastAPIInstrumentor
from opentelemetry.instrumentation.logging import LoggingInstrumentor
from opentelemetry.sdk.metrics import MeterProvider
from opentelemetry.sdk.metrics.export import PeriodicExportingMetricReader
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor

# ---------------------------------------------------------------------------
# Log exporter — still under _logs namespace in the Python SDK
# ---------------------------------------------------------------------------
from opentelemetry.sdk._logs import LoggerProvider, LoggingHandler
from opentelemetry.sdk._logs.export import BatchLogRecordProcessor
from opentelemetry.exporter.otlp.proto.http._log_exporter import (
    OTLPLogExporter,
)
from opentelemetry._logs import set_logger_provider


def _resource() -> Resource:
    return Resource.create(
        {
            "service.name": os.getenv("OTEL_SERVICE_NAME", "spawner"),
            "service.version": "0.1.0",
            "deployment.environment": os.getenv("DEPLOYMENT_ENV", "dev"),
        }
    )


def setup_traces(resource: Resource) -> TracerProvider:
    provider = TracerProvider(resource=resource)
    provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter()))
    trace.set_tracer_provider(provider)
    return provider


def setup_metrics(resource: Resource) -> MeterProvider:
    reader = PeriodicExportingMetricReader(
        OTLPMetricExporter(), export_interval_millis=5000
    )
    provider = MeterProvider(resource=resource, metric_readers=[reader])
    metrics.set_meter_provider(provider)
    return provider


def setup_logs(resource: Resource) -> LoggerProvider:
    provider = LoggerProvider(resource=resource)
    provider.add_log_record_processor(BatchLogRecordProcessor(OTLPLogExporter()))
    set_logger_provider(provider)

    # Bridge Python logging → OTel log records (exported to Loki via collector)
    handler = LoggingHandler(level=logging.DEBUG, logger_provider=provider)
    logging.getLogger().addHandler(handler)

    # Inject trace_id / span_id into every Python log record
    LoggingInstrumentor().instrument(set_logging_format=True)

    return provider


def setup_otel(app):  # noqa: ANN001 — FastAPI type avoided for import order
    """Wire up all three OTel signals and instrument FastAPI."""
    resource = _resource()

    tracer_provider = setup_traces(resource)
    meter_provider = setup_metrics(resource)
    logger_provider = setup_logs(resource)

    FastAPIInstrumentor.instrument_app(app)

    return tracer_provider, meter_provider, logger_provider


def shutdown_otel(tracer_provider, meter_provider, logger_provider) -> None:  # noqa: ANN001
    """Flush pending telemetry on shutdown."""
    tracer_provider.shutdown()
    meter_provider.shutdown()
    logger_provider.shutdown()
