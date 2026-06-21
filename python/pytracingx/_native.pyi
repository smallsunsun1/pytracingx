"""Type stubs for the native ``pytracingx._native`` module."""

from __future__ import annotations

from collections.abc import Iterable, Sequence
from typing import Any

__version__: str

# ─── Sink types ───────────────────────────────────────────────────────────────

class RawOtlp:
    """Typed escape hatch for low-level OTLP knobs.

    Pass an instance to a Sink's `raw_otlp` argument:

        ptx.TraceSink(endpoint="...", raw_otlp=ptx.RawOtlp(compression="gzip"))

    All fields are optional; only set what you need. Adding new fields here
    is a non-breaking change.
    """

    def __init__(self, compression: str | None = ...) -> None: ...

class TraceSink:
    def __init__(
        self,
        endpoint: str,
        protocol: str | None = ...,
        headers: dict[str, str] | None = ...,
        timeout_ms: int | None = ...,
        sampler: str | None = ...,
        sampler_arg: float | None = ...,
        batch_max_queue: int | None = ...,
        batch_max_export: int | None = ...,
        batch_schedule_delay_ms: int | None = ...,
        max_export_timeout_ms: int | None = ...,
        max_attributes_per_span: int | None = ...,
        max_events_per_span: int | None = ...,
        max_links_per_span: int | None = ...,
        max_attributes_per_event: int | None = ...,
        max_attributes_per_link: int | None = ...,
        raw_otlp: RawOtlp | None = ...,
    ) -> None: ...

class MetricSink:
    def __init__(
        self,
        endpoint: str,
        protocol: str | None = ...,
        headers: dict[str, str] | None = ...,
        timeout_ms: int | None = ...,
        export_interval_ms: int | None = ...,
        export_timeout_ms: int | None = ...,
        temporality: str | None = ...,
        raw_otlp: RawOtlp | None = ...,
    ) -> None: ...

class OtlpLogSink:
    def __init__(
        self,
        endpoint: str,
        protocol: str | None = ...,
        headers: dict[str, str] | None = ...,
        timeout_ms: int | None = ...,
        batch_max_queue: int | None = ...,
        batch_max_export: int | None = ...,
        batch_schedule_delay_ms: int | None = ...,
        max_export_timeout_ms: int | None = ...,
        raw_otlp: RawOtlp | None = ...,
    ) -> None: ...

class SlsLogSink:
    def __init__(
        self,
        endpoint: str,
        project: str,
        logstore: str,
        ak_id: str,
        ak_secret: str,
        topic: str = ...,
        source: str = ...,
    ) -> None: ...

# ─── Config ───────────────────────────────────────────────────────────────────

class Config:
    def __init__(
        self,
        service_name: str,
        resource_attributes: dict[str, str] | None = ...,
        console_output: bool = ...,
        console_format: str = ...,
        sinks: Sequence[TraceSink | MetricSink | OtlpLogSink | SlsLogSink] | None = ...,
    ) -> None: ...
    def describe(self) -> dict[str, Any]: ...

# ─── Span / Context ──────────────────────────────────────────────────────────

class Span:
    def __enter__(self) -> Span: ...
    def __exit__(self, exc_type: object, exc_val: object, exc_tb: object) -> bool: ...
    def set_attribute(self, key: str, value: Any) -> None: ...
    def set_status(self, code: str, description: str | None = ...) -> None: ...
    def update_name(self, name: str) -> None: ...
    def add_event(self, name: str, attributes: dict[str, Any] | None = ...) -> None: ...
    def record_exception(self, exception: BaseException, escaped: bool = ...) -> None: ...
    def end(self) -> None: ...
    def trace_id(self) -> str | None: ...
    def span_id(self) -> str | None: ...
    def is_recording(self) -> bool: ...

class ExtractedContext:
    def __enter__(self) -> ExtractedContext: ...
    def __exit__(self, exc_type: object, exc_val: object, exc_tb: object) -> bool: ...
    def restore(self) -> None: ...

# ─── Instruments ──────────────────────────────────────────────────────────────

class Counter:
    def add(self, value: float, attributes: dict[str, Any] | None = ...) -> None: ...

class UpDownCounter:
    def add(self, value: float, attributes: dict[str, Any] | None = ...) -> None: ...

class Histogram:
    def record(self, value: float, attributes: dict[str, Any] | None = ...) -> None: ...

class Gauge:
    def record(self, value: float, attributes: dict[str, Any] | None = ...) -> None: ...

class Meter:
    def counter(
        self, name: str, *, unit: str | None = ..., description: str | None = ...
    ) -> Counter: ...
    def up_down_counter(
        self, name: str, *, unit: str | None = ..., description: str | None = ...
    ) -> UpDownCounter: ...
    def histogram(
        self,
        name: str,
        *,
        unit: str | None = ...,
        description: str | None = ...,
        boundaries: Iterable[float] | None = ...,
    ) -> Histogram: ...
    def gauge(
        self, name: str, *, unit: str | None = ..., description: str | None = ...
    ) -> Gauge: ...

class Logger:
    def trace(self, message: str, *, attributes: dict[str, Any] | None = ...) -> None: ...
    def debug(self, message: str, *, attributes: dict[str, Any] | None = ...) -> None: ...
    def info(self, message: str, *, attributes: dict[str, Any] | None = ...) -> None: ...
    def warn(self, message: str, *, attributes: dict[str, Any] | None = ...) -> None: ...
    def error(self, message: str, *, attributes: dict[str, Any] | None = ...) -> None: ...
    def fatal(self, message: str, *, attributes: dict[str, Any] | None = ...) -> None: ...

# ─── Module-level functions ───────────────────────────────────────────────────

def init(config: Config) -> None: ...
def shutdown() -> None: ...
def force_flush() -> None: ...
def current_trace_context() -> dict[str, str | None]: ...
def start_span(name: str, attributes: dict[str, Any] | None = ..., kind: str = ...) -> Span: ...
def inject_headers(carrier: dict[str, str]) -> None: ...
def extract_headers(carrier: dict[str, str]) -> ExtractedContext: ...
def restore_context(token: object) -> None: ...
def get_meter(name: str) -> Meter: ...
def get_logger(name: str) -> Logger: ...
def emit_log_record(
    name: str,
    message: str,
    *,
    severity_number: int,
    severity_text: str,
    attributes: dict[str, Any] | None = ...,
) -> None: ...
