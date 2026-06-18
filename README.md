# pytracingx

Rust + OpenTelemetry powered Python bindings for **traces**, **metrics** and **logs**, with first-class
support for **Aliyun SLS** and **ARMS** as OTLP backends.

## Architecture

```
Python  ──►  ptx.start_span / ptx.get_logger / ptx.get_meter
                                │
                                ▼
                        tracing crate (Rust)
              ┌──────────────────┴──────────────────┐
              │                                     │
        fmt::Layer                       tracing-opentelemetry
        (terminal)                  + opentelemetry-appender-tracing
                                                    │
                                                    ▼
                            SdkTracerProvider / SdkLoggerProvider / SdkMeterProvider
                                       opentelemetry-otlp
                                                    │
                                                    ▼
                                   SLS / ARMS / any OTLP Collector
```

Each signal (traces, metrics, logs) is independently configured with its own endpoint.
**A signal is enabled if and only if its endpoint is set.**

## Quickstart

```python
import asyncio
import pytracingx as ptx

async def main():
    ptx.init(ptx.Config(
        service_name="payment-svc",
        # Traces + Metrics → ARMS
        traces_endpoint="http://tracing-xxx.arms.aliyuncs.com/.../api/otlp/traces",
        traces_protocol="http/protobuf",
        metrics_endpoint="http://tracing-xxx.arms.aliyuncs.com/.../api/otlp/metrics",
        metrics_protocol="http/protobuf",
        # Logs → SLS (gRPC)
        logs_endpoint="https://my-proj.cn-hangzhou.log.aliyuncs.com:10010",
        logs_protocol="grpc",
        logs_headers={
            "x-sls-otel-project": "my-proj",
            "x-sls-otel-instance-id": "my-inst",
            "x-sls-otel-ak-id": "...",
            "x-sls-otel-ak-secret": "...",
        },
    ))

    meter = ptx.get_meter("payment")
    logger = ptx.get_logger("payment")
    orders = meter.counter("orders_total")

    with ptx.start_span("checkout", kind="server", attributes={"user.id": "u1"}) as span:
        orders.add(1, attributes={"sku": "abc"})
        logger.info("checkout done", attributes={"sku": "abc"})
        span.set_attribute("amount", 12.34)

    ptx.shutdown()

asyncio.run(main())
```

## Console-only mode

```python
# No endpoints → no exporters, just terminal output
ptx.init(ptx.Config(service_name="my-app", console_level="debug"))
```

## Context propagation (server-side)

```python
# Extracts upstream traceparent, auto-restores on exit
with ptx.extract_headers(dict(request.headers)):
    with ptx.start_span("POST /api/orders", kind="server") as span:
        ...
```

## Bridging stdlib `logging`

```python
import logging
from pytracingx.logging import SLSLoggingHandler

logging.basicConfig(level=logging.INFO, handlers=[SLSLoggingHandler()])
logging.getLogger("foo").info("hello from stdlib")
```

## Building from source

```bash
pip install maturin
maturin develop --release
```

Requires Rust >= 1.75 and Python >= 3.9. Wheels are abi3 (`abi3-py39`).

## License

Apache-2.0
