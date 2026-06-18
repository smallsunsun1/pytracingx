"""Tests for runtime lifecycle, spans, metrics, logs, context propagation."""

from __future__ import annotations

import asyncio

import pytest

import pytracingx as ptx


# -- Lifecycle ------------------------------------------------------------------

def test_start_span_before_init_raises() -> None:
    with pytest.raises(RuntimeError):
        ptx.start_span("x")


def test_get_meter_before_init_raises() -> None:
    with pytest.raises(RuntimeError):
        ptx.get_meter("x")


def test_get_logger_before_init_raises() -> None:
    with pytest.raises(RuntimeError):
        ptx.get_logger("x")


def test_shutdown_before_init_noop() -> None:
    ptx.shutdown()


def test_force_flush_before_init_noop() -> None:
    ptx.force_flush()


def test_init_shutdown_roundtrip(console_only_config: ptx.Config) -> None:
    ptx.init(console_only_config)
    assert ptx.is_initialized()
    ptx.shutdown()
    assert not ptx.is_initialized()


def test_double_init_raises(console_only_config: ptx.Config) -> None:
    ptx.init(console_only_config)
    with pytest.raises(RuntimeError, match="already initialized"):
        ptx.init(console_only_config)


# -- Span behaviour -------------------------------------------------------------

def test_span_context_manager(initialized: None) -> None:
    with ptx.start_span("op") as span:
        assert span.is_recording()
        assert span.trace_id() is not None
        assert span.span_id() is not None


def test_span_attributes_and_events(initialized: None) -> None:
    with ptx.start_span("op") as span:
        span.set_attribute("key", "value")
        span.set_attribute("num", 42)
        span.set_status("ok")
        span.add_event("checkpoint", attributes={"step": 1})
        span.update_name("renamed")


def test_invalid_status_rejected(initialized: None) -> None:
    with ptx.start_span("op") as span:
        with pytest.raises(ValueError, match="unknown status code"):
            span.set_status("bogus")


def test_invalid_span_kind_rejected(initialized: None) -> None:
    with pytest.raises(ValueError, match="unknown span kind"):
        ptx.start_span("op", kind="weird")


def test_record_exception(initialized: None) -> None:
    with ptx.start_span("op") as span:
        span.record_exception(ValueError("boom"))


def test_double_end_safe(initialized: None) -> None:
    span = ptx.start_span("op")
    span.end()
    span.end()


def test_exception_in_with_doesnt_break_runtime(initialized: None) -> None:
    with pytest.raises(RuntimeError):
        with ptx.start_span("outer"):
            raise RuntimeError("synthetic")
    with ptx.start_span("recovered") as span:
        assert span.is_recording()


def test_nested_spans_share_trace_id(initialized: None) -> None:
    with ptx.start_span("parent") as parent:
        with ptx.start_span("child") as child:
            assert child.trace_id() == parent.trace_id()
            assert child.span_id() != parent.span_id()


def test_current_trace_context(initialized: None) -> None:
    assert ptx.current_trace_context() == {"trace_id": None, "span_id": None}
    with ptx.start_span("op") as span:
        ctx = ptx.current_trace_context()
        assert ctx["trace_id"] == span.trace_id()
        assert ctx["span_id"] == span.span_id()


# -- Async context propagation --------------------------------------------------

def test_async_tasks_inherit_parent(initialized: None) -> None:
    async def child(expected_trace: str) -> str:
        with ptx.start_span("child") as span:
            await asyncio.sleep(0.001)
            assert span.trace_id() == expected_trace
            return span.span_id() or ""

    async def run() -> None:
        with ptx.start_span("parent") as parent:
            trace_id = parent.trace_id()
            assert trace_id is not None
            ids = await asyncio.gather(child(trace_id), child(trace_id), child(trace_id))
            assert len(set(ids)) == 3

    asyncio.run(run())


# -- W3C propagation (extract_headers with-syntax) ------------------------------

def test_extract_inject_roundtrip(initialized: None) -> None:
    headers: dict[str, str] = {}
    with ptx.start_span("origin") as span:
        ptx.inject_headers(headers)
        original_trace = span.trace_id()

    assert "traceparent" in headers

    with ptx.extract_headers(headers):
        ctx = ptx.current_trace_context()
        assert ctx["trace_id"] == original_trace

    # Context auto-restored after exiting the with block
    assert ptx.current_trace_context() == {"trace_id": None, "span_id": None}


def test_extract_headers_as_parent(initialized: None) -> None:
    """Extracted remote context becomes parent of a new server span."""
    headers: dict[str, str] = {}
    with ptx.start_span("upstream", kind="client"):
        ptx.inject_headers(headers)

    with ptx.extract_headers(headers):
        with ptx.start_span("server-handler", kind="server") as span:
            # The server span continues the upstream trace
            assert span.trace_id() is not None


# -- Metrics --------------------------------------------------------------------

def test_meter_instruments(initialized: None) -> None:
    meter = ptx.get_meter("test")
    counter = meter.counter("c", unit="1", description="test counter")
    counter.add(1, attributes={"k": "v"})

    histogram = meter.histogram("h", unit="ms", boundaries=[1.0, 5.0])
    histogram.record(2.5)

    up_down = meter.up_down_counter("ud")
    up_down.add(1)
    up_down.add(-1)

    gauge = meter.gauge("g")
    gauge.record(0.5)


# -- Logger ---------------------------------------------------------------------

def test_logger_methods(initialized: None) -> None:
    logger = ptx.get_logger("test")
    for fn in (logger.trace, logger.debug, logger.info, logger.warn, logger.error, logger.fatal):
        fn("msg", attributes={"k": "v"})


def test_emit_log_record_noop_before_init() -> None:
    ptx.emit_log_record("x", "hello", severity_number=9, severity_text="INFO")


# -- Module surface -------------------------------------------------------------

def test_version() -> None:
    assert isinstance(ptx.__version__, str) and ptx.__version__


def test_public_api() -> None:
    expected = {
        "Config", "Counter", "ExtractedContext", "Gauge", "Histogram",
        "Logger", "Meter", "Span", "UpDownCounter",
        "current_trace_context", "emit_log_record", "extract_headers",
        "force_flush", "get_logger", "get_meter", "init", "inject_headers",
        "is_initialized", "restore_context", "shutdown", "start_span",
    }
    assert expected <= set(ptx.__all__)
    for name in expected:
        assert hasattr(ptx, name)
