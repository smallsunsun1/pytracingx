"""Comprehensive async example: traces + metrics + logs + context propagation.

Demonstrates all pytracingx capabilities in a single async script:
  - Per-signal endpoint configuration (ARMS for traces/metrics, SLS for logs)
  - Server spans with upstream traceparent extraction (with-syntax)
  - Nested client/internal spans
  - Metrics (counter, histogram, gauge)
  - Logger with structured attributes
  - stdlib logging bridge
  - W3C context injection for downstream calls

Usage:
    python examples/demo.py

Environment variables (for real backends, otherwise uses console-only mode):
    TRACES_ENDPOINT   e.g. http://tracing-xxx.arms.aliyuncs.com/.../api/otlp/traces
    METRICS_ENDPOINT  e.g. http://tracing-xxx.arms.aliyuncs.com/.../api/otlp/metrics
    LOGS_ENDPOINT     e.g. https://proj.cn-hz.log.aliyuncs.com:10010
"""

from __future__ import annotations

import asyncio
import logging
import os
import time

import pytracingx as ptx
from pytracingx.logging import SLSLoggingHandler


async def main() -> None:
    # --- Init: endpoints from env, or console-only if not set ---
    sinks = []
    if os.environ.get("TRACES_ENDPOINT"):
        sinks.append(
            ptx.TraceSink(
                endpoint=os.environ["TRACES_ENDPOINT"],
                protocol="http/protobuf",
            )
        )
    if os.environ.get("METRICS_ENDPOINT"):
        sinks.append(
            ptx.MetricSink(
                endpoint=os.environ["METRICS_ENDPOINT"],
                protocol="http/protobuf",
            )
        )
    if os.environ.get("LOGS_ENDPOINT"):
        sinks.append(
            ptx.SlsLogSink(
                endpoint=os.environ["LOGS_ENDPOINT"],
                project=os.environ.get("SLS_PROJECT"),
                logstore=os.environ.get("SLS_LOGSTORE"),
                ak_id=os.environ.get("SLS_AK_ID", ""),
                ak_secret=os.environ.get("SLS_AK_SECRET", ""),
                topic="",
                source="",
            )
        )

    ptx.init(
        ptx.Config(
            service_name="pytracingx-demo",
            resource_attributes={"deployment.environment": "demo"},
            console_output=True,
            sinks=sinks or None,
        )
    )

    # --- Setup instruments ---
    meter = ptx.get_meter("demo")
    logger = ptx.get_logger("demo")
    requests_total = meter.counter("http_requests_total", description="total requests")
    latency = meter.histogram("http_request_duration_ms", unit="ms")
    active_conns = meter.gauge("active_connections")

    # --- Setup stdlib logging bridge ---
    logging.basicConfig(level=logging.INFO, handlers=[SLSLoggingHandler()])
    stdlib_log = logging.getLogger("demo.stdlib")

    # --- Simulate 3 incoming requests with upstream context propagation ---
    for i in range(3):
        # Simulate upstream gateway creating a trace and passing traceparent
        upstream_headers = await simulate_gateway(i)

        # Handle the request
        await handle_request(
            request_id=i,
            incoming_headers=upstream_headers,
            logger=logger,
            counter=requests_total,
            histogram=latency,
            gauge=active_conns,
        )

    stdlib_log.info("all requests processed", extra={"total": 3})
    ptx.shutdown()
    print("Done. Check your ARMS/SLS console or the terminal output above.")


async def simulate_gateway(request_id: int) -> dict[str, str]:
    """Pretend to be an upstream gateway that starts a trace."""
    with ptx.start_span("gateway-forward", kind="client", attributes={"request_id": request_id}):
        outgoing: dict[str, str] = {}
        ptx.inject_headers(outgoing)
        return outgoing


async def handle_request(
    request_id: int,
    incoming_headers: dict[str, str],
    logger: ptx.Logger,
    counter: ptx.Counter,
    histogram: ptx.Histogram,
    gauge: ptx.Gauge,
) -> None:
    """Full server-side pattern: extract → server span → nested work → metrics."""

    # with-syntax auto-restores context on exit
    with ptx.extract_headers(incoming_headers):
        with ptx.start_span(
            f"POST /api/orders/{request_id}",
            kind="server",
            attributes={"http.method": "POST", "http.url": f"/api/orders/{request_id}"},
        ) as span:
            t0 = time.monotonic()
            gauge.record(1, attributes={"endpoint": "/api/orders"})

            # Auth check (internal)
            with ptx.start_span("auth-check"):
                await asyncio.sleep(0.01)

            # DB call (client)
            with ptx.start_span("INSERT orders", kind="client", attributes={"db.system": "mysql"}):
                await asyncio.sleep(0.03)

            # Cache (client) + inject headers for a hypothetical downstream
            with ptx.start_span("redis SET", kind="client", attributes={"db.system": "redis"}):
                await asyncio.sleep(0.01)
                downstream: dict[str, str] = {}
                ptx.inject_headers(downstream)
                # downstream now has traceparent for the next hop

            elapsed_ms = (time.monotonic() - t0) * 1000
            span.set_attribute("http.status_code", 200)
            span.set_status("ok")

            counter.add(1, attributes={"method": "POST", "status": "200"})
            histogram.record(elapsed_ms, attributes={"method": "POST"})
            gauge.record(0, attributes={"endpoint": "/api/orders"})
            logger.info(
                f"request #{request_id} done",
                attributes={"elapsed_ms": elapsed_ms, "status": 200},
            )


async def demonstrate_async_context_propagation() -> None:
    """Show that spans propagate correctly across asyncio.gather tasks."""
    with ptx.start_span("parent-task") as parent:
        trace_id = parent.trace_id()

        async def child(n: int) -> str:
            with ptx.start_span(f"child-{n}") as child_span:
                await asyncio.sleep(0.01)
                assert child_span.trace_id() == trace_id
                return child_span.span_id() or ""

        span_ids = await asyncio.gather(child(0), child(1), child(2))
        assert len(set(span_ids)) == 3
        parent.add_event("children-done", attributes={"count": 3})


if __name__ == "__main__":
    asyncio.run(main())
