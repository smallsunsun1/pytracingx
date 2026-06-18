"""Shared pytest fixtures.

The Rust `tracing` dispatcher is one-shot per process: once installed it
cannot be replaced. We init exactly once per session and reuse that runtime
for all tests.
"""

from __future__ import annotations

import pytest

import pytracingx as ptx


def _make_config() -> ptx.Config:
    return ptx.Config(
        service_name="ptx-test",
        console_output=False,
        sinks=[
            ptx.TraceSink(
                endpoint="http://127.0.0.1:1",
                sampler="always_on",
                batch_schedule_delay_ms=60_000,
            ),
        ],
    )


@pytest.fixture
def console_only_config() -> ptx.Config:
    return _make_config()


@pytest.fixture
def initialized():
    if not ptx.is_initialized():
        ptx.init(_make_config())
    yield
