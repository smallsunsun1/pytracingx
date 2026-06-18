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
