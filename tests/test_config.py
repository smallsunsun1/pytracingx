"""Tests for Config + Sink validation."""

from __future__ import annotations

import pytest

import pytracingx as ptx


def test_console_only_config() -> None:
    cfg = ptx.Config(service_name="svc")
    d = cfg.describe()
    assert d["service_name"] == "svc"
    assert d["console_output"] is True
    assert d["traces"] is False
    assert d["metrics"] is False


def test_config_with_trace_sink() -> None:
    cfg = ptx.Config(
        service_name="svc",
        sinks=[ptx.TraceSink(endpoint="https://collector:4317")],
    )
    d = cfg.describe()
    assert d["traces"] is True
    assert d["metrics"] is False


def test_config_with_all_sinks() -> None:
    cfg = ptx.Config(
        service_name="svc",
        sinks=[
            ptx.TraceSink(endpoint="http://a/traces", protocol="http/protobuf"),
            ptx.MetricSink(endpoint="http://a/metrics", protocol="http/protobuf"),
            ptx.OtlpLogSink(endpoint="https://sls:10010"),
            ptx.SlsLogSink(
                endpoint="cn-hangzhou.log.aliyuncs.com",
                project="p",
                logstore="l",
                ak_id="ak",
                ak_secret="sk",
            ),
        ],
    )
    d = cfg.describe()
    assert d["traces"] is True
    assert d["metrics"] is True
    assert d["otlp_logs"] is True
    assert d["sls_log"] is True


def test_empty_service_name_rejected() -> None:
    with pytest.raises((ValueError, RuntimeError), match="service_name must not be empty"):
        ptx.Config(service_name="")


def test_trace_sink_empty_endpoint_rejected() -> None:
    with pytest.raises((ValueError, RuntimeError), match="endpoint must not be empty"):
        ptx.TraceSink(endpoint="")


def test_trace_sink_unknown_protocol_rejected() -> None:
    with pytest.raises((ValueError, RuntimeError), match="unknown protocol"):
        ptx.TraceSink(endpoint="x", protocol="thrift")


def test_trace_sink_unknown_sampler_rejected() -> None:
    with pytest.raises((ValueError, RuntimeError), match="unknown sampler"):
        ptx.TraceSink(endpoint="x", sampler="custom")


def test_metric_sink_empty_endpoint_rejected() -> None:
    with pytest.raises((ValueError, RuntimeError), match="endpoint must not be empty"):
        ptx.MetricSink(endpoint="")


def test_sls_log_sink_validation() -> None:
    with pytest.raises((ValueError, RuntimeError), match="project must not be empty"):
        ptx.SlsLogSink(
            endpoint="cn-hangzhou.log.aliyuncs.com",
            project="",
            logstore="l",
            ak_id="ak",
            ak_secret="sk",
        )


def test_duplicate_trace_sink_rejected() -> None:
    with pytest.raises((ValueError, RuntimeError), match="only one TraceSink"):
        ptx.Config(
            service_name="svc",
            sinks=[
                ptx.TraceSink(endpoint="http://a"),
                ptx.TraceSink(endpoint="http://b"),
            ],
        )


def test_unknown_console_format_rejected() -> None:
    with pytest.raises((ValueError, RuntimeError), match="unknown console_format"):
        ptx.Config(service_name="svc", console_format="xml")


@pytest.mark.parametrize("fmt", ["compact", "pretty", "json"])
def test_known_console_formats(fmt: str) -> None:
    cfg = ptx.Config(service_name="svc", console_format=fmt)
    assert cfg.describe()["console_format"] == fmt


def test_repr() -> None:
    cfg = ptx.Config(
        service_name="svc",
        sinks=[ptx.TraceSink(endpoint="x")],
    )
    r = repr(cfg)
    assert "service_name='svc'" in r
    assert "traces=true" in r.lower()


# ── Layer 2: SDK knobs ────────────────────────────────────────────────────────


def test_trace_sink_span_limits_accepted() -> None:
    sink = ptx.TraceSink(
        endpoint="https://collector:4317",
        max_attributes_per_span=256,
        max_events_per_span=256,
        max_links_per_span=64,
        max_attributes_per_event=32,
        max_attributes_per_link=32,
        max_export_timeout_ms=60_000,
    )
    cfg = ptx.Config(service_name="svc", sinks=[sink])
    assert cfg.describe()["traces"] is True


@pytest.mark.parametrize("temp", ["cumulative", "delta", "lowmemory", "LowMemory"])
def test_metric_sink_temporality_accepted(temp: str) -> None:
    cfg = ptx.Config(
        service_name="svc",
        sinks=[ptx.MetricSink(endpoint="http://x/metrics", temporality=temp)],
    )
    assert cfg.describe()["metrics"] is True


def test_metric_sink_unknown_temporality_rejected() -> None:
    with pytest.raises((ValueError, RuntimeError), match="unknown temporality"):
        ptx.MetricSink(endpoint="http://x/metrics", temporality="foo")


def test_metric_sink_export_timeout() -> None:
    cfg = ptx.Config(
        service_name="svc",
        sinks=[ptx.MetricSink(endpoint="http://x/metrics", export_timeout_ms=15_000)],
    )
    assert cfg.describe()["metrics"] is True


def test_otlp_log_sink_export_timeout() -> None:
    cfg = ptx.Config(
        service_name="svc",
        sinks=[ptx.OtlpLogSink(endpoint="https://collector:4317", max_export_timeout_ms=45_000)],
    )
    assert cfg.describe()["otlp_logs"] is True


# ── Layer 3: typed RawOtlp escape hatch ───────────────────────────────────────


@pytest.mark.parametrize("comp", ["gzip", "zstd", "Gzip", "ZSTD"])
def test_raw_otlp_compression_accepted(comp: str) -> None:
    raw = ptx.RawOtlp(compression=comp)
    cfg = ptx.Config(
        service_name="svc",
        sinks=[
            ptx.TraceSink(endpoint="http://x/traces", raw_otlp=raw),
            ptx.MetricSink(endpoint="http://x/metrics", raw_otlp=raw),
            ptx.OtlpLogSink(endpoint="http://x/logs", raw_otlp=raw),
        ],
    )
    d = cfg.describe()
    assert d["traces"] and d["metrics"] and d["otlp_logs"]


def test_raw_otlp_unknown_compression_rejected() -> None:
    with pytest.raises((ValueError, RuntimeError), match="unknown compression"):
        ptx.RawOtlp(compression="snappy")


def test_raw_otlp_no_args_is_ok() -> None:
    """RawOtlp() with no fields = pure default OTel behaviour."""
    raw = ptx.RawOtlp()
    cfg = ptx.Config(
        service_name="svc",
        sinks=[ptx.TraceSink(endpoint="http://x/traces", raw_otlp=raw)],
    )
    assert cfg.describe()["traces"] is True


def test_raw_otlp_repr() -> None:
    assert repr(ptx.RawOtlp()) == "RawOtlp()"
    assert repr(ptx.RawOtlp(compression="gzip")) == "RawOtlp(compression='gzip')"
    assert repr(ptx.RawOtlp(compression="zstd")) == "RawOtlp(compression='zstd')"


def test_sink_raw_otlp_none_is_ok() -> None:
    sink = ptx.TraceSink(endpoint="http://x/traces", raw_otlp=None)
    cfg = ptx.Config(service_name="svc", sinks=[sink])
    assert cfg.describe()["traces"] is True


def test_sink_default_options_use_otel_defaults() -> None:
    """All knobs are Option<None>; the Rust runtime should fall back to
    opentelemetry-rust defaults without any user overrides."""
    sink = ptx.TraceSink(endpoint="http://x/traces")
    cfg = ptx.Config(service_name="svc", sinks=[sink])
    assert cfg.describe()["traces"] is True
