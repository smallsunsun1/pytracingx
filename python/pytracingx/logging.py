"""Bridge between the Python standard library `logging` module and pytracingx.

Add :class:`SLSLoggingHandler` to a logger to forward every log record into
the active OpenTelemetry log pipeline.

Example::

    import logging
    from pytracingx.logging import SLSLoggingHandler

    logging.basicConfig(level=logging.INFO, handlers=[SLSLoggingHandler()])
    logging.getLogger("foo").info("hello from stdlib")
"""

from __future__ import annotations

import logging
from typing import Any

from . import _native

__all__ = ["SLSLoggingHandler", "install"]


# Mapping from stdlib logging levels to OTel severity numbers.
# https://opentelemetry.io/docs/specs/otel/logs/data-model/#field-severitynumber
_LEVEL_TO_SEVERITY: dict[int, tuple[int, str]] = {
    logging.NOTSET: (1, "TRACE"),
    logging.DEBUG: (5, "DEBUG"),
    logging.INFO: (9, "INFO"),
    logging.WARNING: (13, "WARN"),
    logging.ERROR: (17, "ERROR"),
    logging.CRITICAL: (21, "FATAL"),
}


def _severity_for(levelno: int) -> tuple[int, str]:
    if levelno >= logging.CRITICAL:
        return _LEVEL_TO_SEVERITY[logging.CRITICAL]
    if levelno >= logging.ERROR:
        return _LEVEL_TO_SEVERITY[logging.ERROR]
    if levelno >= logging.WARNING:
        return _LEVEL_TO_SEVERITY[logging.WARNING]
    if levelno >= logging.INFO:
        return _LEVEL_TO_SEVERITY[logging.INFO]
    if levelno >= logging.DEBUG:
        return _LEVEL_TO_SEVERITY[logging.DEBUG]
    return _LEVEL_TO_SEVERITY[logging.NOTSET]


class SLSLoggingHandler(logging.Handler):
    """Forwards ``logging.LogRecord`` instances into pytracingx's log pipeline.

    The handler is *non-blocking*: emit() pushes records into the in-process
    Rust batch processor, which exports asynchronously via OTLP. If pytracingx
    has not been initialized yet, emit() silently no-ops so import-time logs
    don't crash the application.
    """

    def __init__(self, level: int = logging.NOTSET) -> None:
        super().__init__(level=level)

    def emit(self, record: logging.LogRecord) -> None:  # noqa: D401
        if not _native.is_initialized():
            return
        try:
            severity, severity_text = _severity_for(record.levelno)
            attrs: dict[str, Any] = {
                "logger.name": record.name,
                "code.filepath": record.pathname,
                "code.function": record.funcName,
                "code.lineno": record.lineno,
                "thread.id": record.thread,
                "thread.name": record.threadName,
                "process.pid": record.process,
            }
            if record.exc_info:
                attrs["exception.type"] = (
                    record.exc_info[0].__name__ if record.exc_info[0] else "Exception"
                )
                attrs["exception.message"] = str(record.exc_info[1] or "")
            # Preserve user-provided extras that fit primitive types.
            standard = vars(logging.LogRecord("", 0, "", 0, "", (), None))
            for key, value in record.__dict__.items():
                if key in standard or key in {"message", "asctime"}:
                    continue
                if isinstance(value, (str, int, float, bool)) or value is None:
                    attrs.setdefault(key, value)

            _native.emit_log_record(
                record.name,
                self.format(record),
                severity_number=severity,
                severity_text=severity_text,
                attributes={k: v for k, v in attrs.items() if v is not None},
            )
        except Exception:  # noqa: BLE001 - handlers must never propagate
            self.handleError(record)


def install(level: int = logging.INFO, logger: logging.Logger | None = None) -> SLSLoggingHandler:
    """Convenience helper: attach an :class:`SLSLoggingHandler` to ``logger``.

    Defaults to the root logger and INFO level. Returns the handler so callers
    can detach it again later if they want.
    """
    handler = SLSLoggingHandler(level=level)
    target = logger if logger is not None else logging.getLogger()
    target.addHandler(handler)
    if target.level == logging.NOTSET or target.level > level:
        target.setLevel(level)
    return handler
