"""FastAPI middleware that creates a server span per request and propagates context.

Demonstrates the typical web-service integration pattern:

  1. Extract W3C traceparent / tracestate from incoming HTTP headers
  2. Open a server-kind span around the request
  3. Tag it with route / method / status_code
  4. Propagate the context to any downstream HTTP/gRPC client calls
  5. Inject traceparent into the response headers (optional, helps debugging)

Run:
    pip install fastapi uvicorn
    python examples/fastapi_middleware.py

Then:
    curl -i http://127.0.0.1:8000/ping
    # Look at the response 'traceparent' header — that's the trace ID for this request.
"""

from __future__ import annotations

import os
import time

from fastapi import FastAPI, Request
from starlette.middleware.base import BaseHTTPMiddleware

import pytracingx as ptx


def _init_tracing() -> None:
    if ptx.is_initialized():
        return

    sinks = []
    if traces_endpoint := os.environ.get("OTLP_TRACES_ENDPOINT"):
        sinks.append(
            ptx.TraceSink(
                endpoint=traces_endpoint,
                protocol="http/protobuf",
                sampler="parent_based_traceid_ratio",
                sampler_arg=1.0,
            )
        )
    if metrics_endpoint := os.environ.get("OTLP_METRICS_ENDPOINT"):
        sinks.append(
            ptx.MetricSink(
                endpoint=metrics_endpoint,
                protocol="http/protobuf",
                export_interval_ms=15_000,
            )
        )

    ptx.init(
        ptx.Config(
            service_name=os.environ.get("SERVICE_NAME", "fastapi-demo"),
            resource_attributes={"deployment.environment": "demo"},
            sinks=sinks,
            console_output=not sinks,  # If no remote sink, fall back to stderr
            console_level="info",
        )
    )


class TracingMiddleware(BaseHTTPMiddleware):
    """Wraps every request in a pytracingx server span.

    The route template (e.g. `/users/{id}`) is preferred over the raw path
    so high-cardinality URLs don't blow up the trace index.
    """

    async def dispatch(self, request: Request, call_next):
        carrier = {k.lower(): v for k, v in request.headers.items()}
        with ptx.extract_headers(carrier):
            route = request.scope.get("route")
            name = f"{request.method} {route.path if route else request.url.path}"

            with ptx.start_span(
                name,
                kind="server",
                attributes={
                    "http.request.method": request.method,
                    "url.path": request.url.path,
                    "url.scheme": request.url.scheme,
                    "client.address": request.client.host if request.client else "",
                },
            ) as span:
                t0 = time.monotonic()
                try:
                    response = await call_next(request)
                except Exception as exc:
                    span.record_exception(exc)
                    raise
                span.set_attribute("http.response.status_code", response.status_code)
                span.set_attribute(
                    "http.server.duration_ms",
                    (time.monotonic() - t0) * 1000,
                )
                if response.status_code >= 500:
                    span.set_status("error", description=f"HTTP {response.status_code}")

                # Echo trace context back so curl/Postman users can see it
                outgoing: dict[str, str] = {}
                ptx.inject_headers(outgoing)
                for k, v in outgoing.items():
                    response.headers[k] = v
                return response


_init_tracing()
app = FastAPI()
app.add_middleware(TracingMiddleware)

_meter = ptx.get_meter("fastapi-demo")
_request_counter = _meter.counter("http_requests_total")


@app.get("/ping")
async def ping():
    _request_counter.add(1, attributes={"route": "/ping"})
    # Nested span demonstrates child propagation
    with ptx.start_span("compute-pong", attributes={"depth": 1}):
        await _fake_db_call()
    return {"pong": True, "trace_id": ptx.current_trace_context()["trace_id"]}


async def _fake_db_call() -> None:
    with ptx.start_span("db.query", attributes={"db.system": "postgres"}):
        import asyncio

        await asyncio.sleep(0.005)


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="127.0.0.1", port=8000)
