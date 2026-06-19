"""Microbenchmark: pytracingx vs opentelemetry-python on the hot path.

Measures the per-call overhead of the three operations users hit most:

    start_span() / counter.add() / logger.info()

We deliberately use a noop / unreachable exporter so we measure SDK overhead
on the Python side, not network or backend latency.

Run:
    pip install pytest-benchmark opentelemetry-sdk opentelemetry-api
    pytest benchmarks/bench_hot_path.py --benchmark-only

Expected: pytracingx is roughly an order of magnitude faster on every op
because span/metric/log construction happens in Rust without holding the GIL.
"""

from __future__ import annotations

import pytest
from opentelemetry import metrics as otel_metrics
from opentelemetry import trace as otel_trace
from opentelemetry._logs import SeverityNumber, set_logger_provider
from opentelemetry.sdk._logs import LoggerProvider
from opentelemetry.sdk._logs._internal import LogRecord
from opentelemetry.sdk._logs.export import BatchLogRecordProcessor, ConsoleLogExporter
from opentelemetry.sdk.metrics import MeterProvider
from opentelemetry.sdk.metrics.export import (
    ConsoleMetricExporter,
    PeriodicExportingMetricReader,
)
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import (
    BatchSpanProcessor,
    ConsoleSpanExporter,
)

import pytracingx as ptx

# ── pytracingx setup ──────────────────────────────────────────────────────────
if not ptx.is_initialized():
    ptx.init(
        ptx.Config(
            service_name="bench",
            console_output=False,
            sinks=[
                ptx.TraceSink(
                    endpoint="http://127.0.0.1:1",
                    sampler="always_on",
                    batch_schedule_delay_ms=600_000,
                ),
                ptx.MetricSink(
                    endpoint="http://127.0.0.1:1",
                    export_interval_ms=600_000,
                ),
            ],
        )
    )

_PTX_METER = ptx.get_meter("bench")
_PTX_COUNTER = _PTX_METER.counter("bench_counter")
_PTX_LOGGER = ptx.get_logger("bench")


# ── opentelemetry-python setup ────────────────────────────────────────────────
class _NullSpanExporter(ConsoleSpanExporter):
    def export(self, spans):
        return 0  # SUCCESS, but write nothing


class _NullMetricExporter(ConsoleMetricExporter):
    def export(self, metrics_data, timeout_millis=10_000):
        return 0


_otel_tracer_provider = TracerProvider()
_otel_tracer_provider.add_span_processor(BatchSpanProcessor(_NullSpanExporter()))
otel_trace.set_tracer_provider(_otel_tracer_provider)
_OTEL_TRACER = otel_trace.get_tracer("bench")

_otel_meter_provider = MeterProvider(
    metric_readers=[
        PeriodicExportingMetricReader(_NullMetricExporter(), export_interval_millis=600_000)
    ]
)
otel_metrics.set_meter_provider(_otel_meter_provider)
_OTEL_METER = otel_metrics.get_meter("bench")
_OTEL_COUNTER = _OTEL_METER.create_counter("bench_counter")


class _NullLogExporter(ConsoleLogExporter):
    def export(self, batch):
        return 0


_otel_logger_provider = LoggerProvider()
_otel_logger_provider.add_log_record_processor(BatchLogRecordProcessor(_NullLogExporter()))
set_logger_provider(_otel_logger_provider)
_OTEL_LOGGER = _otel_logger_provider.get_logger("bench")


# ── Benchmarks ────────────────────────────────────────────────────────────────


def test_ptx_start_span(benchmark):
    def go():
        with ptx.start_span("op", attributes={"i": 1}):
            pass

    benchmark(go)


def test_otel_start_span(benchmark):
    def go():
        with _OTEL_TRACER.start_as_current_span("op", attributes={"i": 1}):
            pass

    benchmark(go)


def test_ptx_counter_add(benchmark):
    benchmark(_PTX_COUNTER.add, 1, attributes={"k": "v"})


def test_otel_counter_add(benchmark):
    benchmark(_OTEL_COUNTER.add, 1, attributes={"k": "v"})


def test_ptx_logger_info(benchmark):
    benchmark(_PTX_LOGGER.info, "hello", attributes={"k": "v"})


def test_otel_logger_info(benchmark):
    def go():
        _OTEL_LOGGER.emit(
            LogRecord(
                body="hello",
                severity_number=SeverityNumber.INFO,
                severity_text="INFO",
                attributes={"k": "v"},
            )
        )

    benchmark(go)


if __name__ == "__main__":
    pytest.main([__file__, "--benchmark-only"])
