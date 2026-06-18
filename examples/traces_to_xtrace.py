"""Send traces, metrics, and logs to ARMS OpenTelemetry endpoint.

ARMS (可观测链路 OpenTelemetry 版) provides token-authenticated OTLP HTTP
endpoints for all three signals. No separate AK/SK or instance_id is needed —
the authentication token is embedded in the URL path.

Required env vars:
  ARMS_OTLP_ENDPOINT    - ARMS OTLP base endpoint (without /traces, /metrics, /logs suffix),
                          e.g. http://tracing-cn-wulanchabu.arms.aliyuncs.com/adapt_xxx@yyy/api/otlp
"""

from __future__ import annotations

import os
import time

import pytracingx as ptx


def main() -> None:
    arms_base = os.environ["ARMS_OTLP_ENDPOINT"].rstrip("/")

    ptx.init(
        ptx.Config(
            endpoint=f"{arms_base}/traces",
            service_name=os.environ.get("SERVICE_NAME", "pytracingx-xtrace-demo"),
            protocol="http/protobuf",
            resource_attributes={"deployment.environment": "demo"},
            traces_endpoint=f"{arms_base}/traces",
            metrics_endpoint=f"{arms_base}/metrics",
            logs_endpoint=f"{arms_base}/logs",
            enable_logs=False,
        )
    )

    meter = ptx.get_meter("xtrace-demo")
    logger = ptx.get_logger("xtrace-demo")
    latency = meter.histogram("request_duration_ms", unit="ms")

    for i in range(5):
        with ptx.start_span("handle-request", attributes={"request_id": i}, kind="server") as span:
            logger.info(f"handling request #{i}", attributes={"request_id": i})
            t0 = time.monotonic()
            time.sleep(0.08)

            with ptx.start_span("db-query", attributes={"table": "orders"}):
                time.sleep(0.03)

            with ptx.start_span("cache-lookup", attributes={"hit": i % 2 == 0}):
                time.sleep(0.01)

            elapsed_ms = (time.monotonic() - t0) * 1000
            latency.record(elapsed_ms)
            logger.info(
                f"request #{i} completed",
                attributes={"elapsed_ms": elapsed_ms},
            )
            span.set_attribute("http.status_code", 200)

    ctx = ptx.current_trace_context()
    print(f"Last trace_id: {ctx['trace_id']}")
    print(f"All signals -> ARMS ({arms_base})")

    ptx.shutdown()


if __name__ == "__main__":
    main()
