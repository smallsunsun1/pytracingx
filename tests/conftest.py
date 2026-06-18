"""Shared pytest fixtures."""

from __future__ import annotations

import pytest

import pytracingx as ptx


@pytest.fixture
def console_only_config() -> ptx.Config:
    return ptx.Config(service_name="ptx-test", console_output=False)


@pytest.fixture
def initialized(console_only_config: ptx.Config):
    if ptx.is_initialized():
        ptx.shutdown()
    ptx.init(console_only_config)
    try:
        yield
    finally:
        if ptx.is_initialized():
            ptx.shutdown()


@pytest.fixture(autouse=True)
def _isolate():
    if ptx.is_initialized():
        ptx.shutdown()
    yield
    if ptx.is_initialized():
        ptx.shutdown()
