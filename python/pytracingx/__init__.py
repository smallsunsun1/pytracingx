"""Rust + OpenTelemetry powered Python bindings."""

from __future__ import annotations

from . import _native
from ._native import (
    Config,
    Counter,
    ExtractedContext,
    Gauge,
    Histogram,
    Logger,
    Meter,
    MetricSink,
    OtlpLogSink,
    RawOtlp,
    SlsLogSink,
    Span,
    TraceSink,
    UpDownCounter,
    current_trace_context,
    emit_log_record,
    extract_headers,
    force_flush,
    get_logger,
    get_meter,
    init,
    inject_headers,
    is_initialized,
    restore_context,
    shutdown,
    start_span,
)

__all__ = [
    "Config",
    "Counter",
    "ExtractedContext",
    "Gauge",
    "Histogram",
    "Logger",
    "Meter",
    "MetricSink",
    "OtlpLogSink",
    "RawOtlp",
    "SlsLogSink",
    "Span",
    "TraceSink",
    "UpDownCounter",
    "current_trace_context",
    "emit_log_record",
    "extract_headers",
    "force_flush",
    "get_logger",
    "get_meter",
    "init",
    "inject_headers",
    "is_initialized",
    "restore_context",
    "shutdown",
    "start_span",
]

__version__ = _native.__version__
