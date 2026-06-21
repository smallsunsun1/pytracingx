"""Concurrent throughput benchmark: pytracingx vs opentelemetry-python.

The hot-path microbench (`bench_hot_path.py`) measures single-call latency.
This file measures **what happens under load** — many threads hammering the
SDK at the same time, which is closer to a real high-QPS web service or
inference engine.

Why this matters: Python's GIL means even with N threads, only one runs
Python bytecode at a time. Any SDK that holds the GIL during span
creation/serialization becomes a global throughput bottleneck. pytracingx
drops the GIL after argument conversion, so its export work happens on Rust
threads truly in parallel.

Run:
    pip install pytest-benchmark opentelemetry-sdk opentelemetry-api
    pytest benchmarks/bench_concurrent.py --benchmark-only

Result interpretation:
    - Single-thread numbers → call latency
    - Multi-thread aggregate ops/sec → effective throughput under contention
    - Ratio of (multi-thread ops / single-thread ops) → how well it scales
      with concurrent producers (1.0 = no scaling, 4+ = good GIL release)
"""

from __future__ import annotations

import threading
from concurrent.futures import ThreadPoolExecutor

import pytest
from opentelemetry import metrics as otel_metrics
from opentelemetry import trace as otel_trace
from opentelemetry._logs import set_logger_provider
from opentelemetry.sdk._logs import LoggerProvider
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

# ── Setup (same as bench_hot_path.py) ─────────────────────────────────────────

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


class _NullSpanExporter(ConsoleSpanExporter):
    def export(self, spans):
        return 0


class _NullMetricExporter(ConsoleMetricExporter):
    def export(self, metrics_data, timeout_millis=10_000):
        return 0


class _NullLogExporter(ConsoleLogExporter):
    def export(self, batch):
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
_OTEL_COUNTER = otel_metrics.get_meter("bench").create_counter("bench_counter")

_otel_logger_provider = LoggerProvider()
_otel_logger_provider.add_log_record_processor(BatchLogRecordProcessor(_NullLogExporter()))
set_logger_provider(_otel_logger_provider)


# ── Workload functions ───────────────────────────────────────────────────────


def _ptx_span_burst(n: int) -> None:
    for i in range(n):
        with ptx.start_span("op", attributes={"i": i}):
            pass


def _otel_span_burst(n: int) -> None:
    for i in range(n):
        with _OTEL_TRACER.start_as_current_span("op", attributes={"i": i}):
            pass


def _ptx_counter_burst(n: int) -> None:
    for i in range(n):
        _PTX_COUNTER.add(1, attributes={"i": i % 8})


def _otel_counter_burst(n: int) -> None:
    for i in range(n):
        _OTEL_COUNTER.add(1, attributes={"i": i % 8})


# ── Concurrent runner ────────────────────────────────────────────────────────

THREADS = 8
ITERS_PER_THREAD = 500


def _run_concurrent(workload):
    barrier = threading.Barrier(THREADS)

    def worker():
        barrier.wait()  # Maximize contention by starting together
        workload(ITERS_PER_THREAD)

    with ThreadPoolExecutor(max_workers=THREADS) as ex:
        futures = [ex.submit(worker) for _ in range(THREADS)]
        for f in futures:
            f.result()


# ── Concurrent benchmarks ────────────────────────────────────────────────────


@pytest.mark.benchmark(group="span-concurrent-8t")
def test_ptx_start_span_concurrent(benchmark):
    """8 threads × 500 spans each, measured as a single batch."""
    benchmark(_run_concurrent, _ptx_span_burst)


@pytest.mark.benchmark(group="span-concurrent-8t")
def test_otel_start_span_concurrent(benchmark):
    benchmark(_run_concurrent, _otel_span_burst)


@pytest.mark.benchmark(group="counter-concurrent-8t")
def test_ptx_counter_add_concurrent(benchmark):
    benchmark(_run_concurrent, _ptx_counter_burst)


@pytest.mark.benchmark(group="counter-concurrent-8t")
def test_otel_counter_add_concurrent(benchmark):
    benchmark(_run_concurrent, _otel_counter_burst)


if __name__ == "__main__":
    pytest.main([__file__, "--benchmark-only"])
