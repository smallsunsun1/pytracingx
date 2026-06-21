"""Shared pytest fixtures."""

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


_init_done = False


@pytest.fixture
def initialized():
    global _init_done
    if not _init_done:
        ptx.init(_make_config())
        _init_done = True
    yield
