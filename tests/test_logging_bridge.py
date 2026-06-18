"""Tests for the stdlib logging bridge."""

from __future__ import annotations

import logging

import pytest

from pytracingx.logging import SLSLoggingHandler, _severity_for, install


@pytest.mark.parametrize(
    "level,expected",
    [
        (logging.DEBUG, (5, "DEBUG")),
        (logging.INFO, (9, "INFO")),
        (logging.WARNING, (13, "WARN")),
        (logging.ERROR, (17, "ERROR")),
        (logging.CRITICAL, (21, "FATAL")),
        (logging.NOTSET, (1, "TRACE")),
    ],
)
def test_severity_mapping(level: int, expected: tuple[int, str]) -> None:
    assert _severity_for(level) == expected


def test_handler_emits_after_init(initialized: None) -> None:
    logger = logging.getLogger("ptx.test.bridge")
    logger.handlers = [SLSLoggingHandler(level=logging.DEBUG)]
    logger.setLevel(logging.DEBUG)
    logger.propagate = False
    try:
        logger.info("hello")
        try:
            raise RuntimeError("boom")
        except RuntimeError:
            logger.exception("oops")
    finally:
        logger.handlers = []


def test_install_helper() -> None:
    target = logging.getLogger("ptx.test.install")
    target.handlers = []
    handler = install(level=logging.WARNING, logger=target)
    try:
        assert handler in target.handlers
        assert target.level == logging.WARNING
    finally:
        target.handlers = []
        target.setLevel(logging.NOTSET)
