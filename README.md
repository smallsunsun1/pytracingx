# pytracingx

[中文版 README](README.zh-CN.md)

Rust + OpenTelemetry powered Python bindings for **traces**, **metrics** and **logs**, with first-class
support for **Aliyun SLS** and **ARMS** as OTLP backends.

## Why pytracingx over Python OpenTelemetry SDK?

| | pytracingx (Rust) | opentelemetry-python |
|---|---|---|
| **Performance** | Serialization (protobuf), compression (gzip/lz4), batching, network I/O all happen on Rust native threads — **never holds the GIL** | Every export step runs on Python threads under the GIL; 5–15% CPU overhead is measurable under heavy span/metric load |
| **Memory** | Span/metric data structures live on the Rust heap, zero Python object overhead | Each span is a Python object with dict attributes; significant GC pressure under high traffic |
| **Startup** | Single `.so` file, `import pytracingx` ~15ms | Pulls in `opentelemetry-api` + `-sdk` + `-exporter-otlp` + grpcio/protobuf and a dozen others, cold start 200–500ms |
| **Dependencies** | Zero Python runtime deps (everything compiled into the native module) | Drags in grpcio (C build), protobuf, googleapis-common-protos; wheel size > 50MB |
| **GIL friendliness** | `start_span()` / `counter.add()` / `logger.info()` only do FFI argument conversion (μs), then drop the GIL | Python SDK's `start_span` does context management, attribute serialization and sampler decisions in Python (10–50μs) holding the GIL throughout |
| **Async safety** | Span context lives in `contextvars`, naturally inherited by `asyncio.Task`; the Rust `tracing` layer doesn't depend on the Python event loop | `BatchSpanProcessor` spawns extra daemon threads and uses `threading.Event`; occasional races when paired with uvloop |
| **Console output** | `tracing`'s `fmt::Layer` unifies span begin/end + log events with compact / pretty / json formats | Needs a separate `ConsoleSpanExporter` plus a `logging` handler; two inconsistent formats |
| **Native SLS** | Built-in `SlsLogSink` writes to any logstore directly (protobuf + lz4 + HMAC signing all done in Rust) | Requires `aliyun-log-python-sdk` or hand-rolled HTTP upload |
| **Single wheel** | One `abi3-py39` wheel covers Python 3.9–3.13+ across all platforms (manylinux/macOS/Windows) | grpcio wheel must be built per Python version per platform |

### When to use it

- **High-QPS services** (>1000 RPS): Rust handles batch export in the background; per-call overhead < 1μs on the Python side
- **Latency-sensitive workloads**: span export never blocks the GIL
- **Containers / serverless**: single-file deploy, fast cold start, no grpcio compilation
- **Aliyun full stack**: traces → ARMS, metrics → ARMS, logs → any SLS logstore — one `Config` covers all

## Observability for the AI era — built for LLM inference

In LLM / large model inference, **any CPU-side observability cost translates directly into GPU underutilization**.
When the Python main thread is busy with trace serialization, log formatting and metric aggregation, GPU kernel
launches, KV-cache scheduling and batch composition all get delayed. Symptoms include:

- **TTFT (Time To First Token) jitter**: span export blocks the GIL and request enqueue is delayed
- **GPU bubbles**: the CPU can't feed data fast enough, GPU SMs idle, throughput drops
- **Degraded batch scheduling**: vLLM / SGLang schedulers depend on a low-latency event loop; daemon threads from the Python SDK fight uvloop and break continuous batching

pytracingx's Rust-native design fits these scenarios naturally:

| Pain point | Traditional Python SDK | pytracingx |
|---|---|---|
| **GPU scheduling blocked by GIL** | `start_span` holds GIL for 10–50μs, preempting inference steps | FFI argument copy then GIL released, μs-level return |
| **Token-level tracing overhead** | Per-token spans create heavy Python GC pressure | Spans allocated on the Rust heap, zero impact on Python GC |
| **Prompt/completion log volume** | Large payloads serialized in Python slow down the main loop | Protobuf + async batching all run inside the Rust tokio runtime |
| **Multi-GPU / multi-process deployment** | Each worker loads the full Python OTel stack, doubling memory | Single `.so`, shared abi3 wheel, < 5MB resident overhead beyond GPU memory |

### Inference service integration pattern

```python
# At the entry point of vLLM / SGLang / TGI etc.
with ptx.start_span("llm.inference", attributes={
    "llm.model": "qwen2.5-72b",
    "llm.prompt_tokens": prompt_len,
    "gen.batch_size": batch_size,
}) as span:
    output = engine.generate(prompts)        # GPU work runs undisturbed
    span.set_attribute("llm.completion_tokens", output.usage.completion_tokens)
    span.set_attribute("llm.ttft_ms", output.metrics.first_token_ms)
```

Suggested observation dimensions:

- **Trace**: end-to-end request → tokenize → schedule → prefill → decode → detokenize
- **Metrics**: TTFT / TPOT (Time Per Output Token) / GPU SM utilization / KV-cache hit rate
- **Logs**: sample prompt / completion to SLS for offline RAG / fine-tuning data

**Core idea**: observability should be part of the AI infrastructure, not a performance tax.

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
                                    + SlsLogLayer (native SLS)
                                                    │
                                                    ▼
                            SdkTracerProvider / SdkLoggerProvider / SdkMeterProvider
                                       opentelemetry-otlp (async reqwest)
                                                    │
                                                    ▼
                                   SLS / ARMS / any OTLP Collector
```

Each signal is configured via a **Sink** object. A signal is enabled if its Sink appears in the `sinks` list.

## Installation

```bash
pip install pytracingx
```

Pre-built wheels are available on PyPI for:

- **Linux** x86_64 / aarch64 — `manylinux_2_28` (glibc) and `musllinux_1_2` (Alpine)
- **macOS** — universal2 (Intel + Apple Silicon)
- **Python** 3.9, 3.10, 3.11, 3.12, 3.13+ (single abi3 wheel)

No Rust toolchain or system OpenSSL is required at install time — everything is statically linked into the wheel.

## Quickstart

```python
import asyncio
import pytracingx as ptx

async def main():
    ptx.init(ptx.Config(
        service_name="payment-svc",
        resource_attributes={"deployment.environment": "prod"},
        sinks=[
            # Traces + Metrics → ARMS
            ptx.TraceSink(
                endpoint="http://tracing-xxx.arms.aliyuncs.com/.../api/otlp/traces",
                protocol="http/protobuf",
                sampler="parent_based_traceid_ratio",
                sampler_arg=0.5,
            ),
            ptx.MetricSink(
                endpoint="http://tracing-xxx.arms.aliyuncs.com/.../api/otlp/metrics",
                protocol="http/protobuf",
                export_interval_ms=30_000,
            ),
            # Logs → SLS native (any logstore)
            ptx.SlsLogSink(
                endpoint="cn-hangzhou.log.aliyuncs.com",
                project="my-proj",
                logstore="app-logs",
                ak_id="...",
                ak_secret="...",
            ),
        ],
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

## Sink Types

| Sink | Backend | Protocol | Use Case |
|------|---------|----------|----------|
| `TraceSink` | Any OTLP collector | gRPC / HTTP | Distributed tracing spans |
| `MetricSink` | Any OTLP collector | gRPC / HTTP | Counters, histograms, gauges |
| `OtlpLogSink` | Any OTLP collector | gRPC / HTTP | Logs via OTLP (lands in the trace instance's `-logs` logstore on SLS) |
| `SlsLogSink` | Aliyun SLS native API | HTTPS | Logs to **any** SLS logstore (not limited to the trace instance) |

## Console-only mode

```python
# No sinks → no network, just terminal output
ptx.init(ptx.Config(service_name="my-app", console_level="debug"))
```

## Context propagation (server side)

```python
# with-syntax auto-restores context on exit
with ptx.extract_headers(dict(request.headers)):
    with ptx.start_span("POST /api/orders", kind="server") as span:
        ...
```

## Examples and benchmarks

- `examples/demo.py` — comprehensive async example covering traces + metrics + logs + context propagation
- `examples/fastapi_middleware.py` — FastAPI middleware that creates a server span per request and propagates W3C trace context
- `examples/traces_to_xtrace.py` — route traces to ARMS while metrics/logs go to SLS
- `benchmarks/` — microbenchmarks comparing pytracingx vs `opentelemetry-python` on the hot path

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

Requires Rust >= 1.85 (edition 2024) and Python >= 3.9. Wheels are abi3 (`abi3-py39`).

## License

MIT
