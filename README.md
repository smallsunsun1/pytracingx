# pytracingx

Rust + OpenTelemetry powered Python bindings for **traces**, **metrics** and **logs**, with first-class
support for **Aliyun SLS** and **ARMS** as OTLP backends.

## Why pytracingx over Python OpenTelemetry SDK?

| | pytracingx (Rust) | opentelemetry-python |
|---|---|---|
| **性能** | 序列化 (protobuf)、压缩 (gzip/lz4)、批处理、网络 I/O 全部在 Rust 原生线程完成,**不持有 GIL** | 所有导出逻辑在 Python 线程执行,受 GIL 限制,大量 span/metric 时可测到 5-15% CPU 开销 |
| **内存** | span/metric 数据结构在 Rust 堆上,零 Python 对象开销 | 每个 span 是一个 Python 对象,属性是 dict,大流量下 GC 压力显著 |
| **启动速度** | 单个 `.so` 文件,`import pytracingx` ~15ms | 需要 `opentelemetry-api` + `opentelemetry-sdk` + `opentelemetry-exporter-otlp` + grpcio/protobuf 等十余个包,冷启动 200-500ms |
| **依赖** | Python 侧零运行时依赖(全部编译进 native module) | 拉入 grpcio(C 编译)、protobuf、googleapis-common-protos 等,wheel 体积 >50MB |
| **GIL 友好** | `start_span()` / `counter.add()` / `logger.info()` 调用只做 FFI 入参转换(μs 级),然后立即释放 GIL 回到 Python | SDK 的 `start_span` 在 Python 侧做 context 管理、属性序列化、sampler 判断(10-50μs),全程持有 GIL |
| **异步安全** | `contextvars` 管理 span context,`asyncio.Task` 天然继承;Rust 的 `tracing` 层不依赖 Python event loop | Python SDK 的 `BatchSpanProcessor` 开额外 daemon thread 并用 `threading.Event` 同步,与 uvloop 组合时偶有竞态 |
| **终端输出** | `tracing` 的 `fmt::Layer` 统一输出 span 开始/结束 + 日志事件,格式可选 compact/pretty/json | 需要额外配置 `ConsoleSpanExporter` + `logging` handler,两套格式不统一 |
| **Aliyun SLS 原生** | 内置 `SlsLogSink` 直接写任意 logstore(protobuf + lz4 + HMAC 签名全在 Rust 完成) | 需要额外引入 `aliyun-log-python-sdk` 或自行 HTTP 上报 |
| **单 wheel** | `abi3-py39` 一份 wheel 覆盖 Python 3.9-3.13+,所有平台(manylinux/macOS/Windows) | 每个 Python 版本 + 平台单独编译 grpcio wheel |

### 适用场景

- **高 QPS 服务**(>1000 RPS):Rust 的 batch export 在后台完成,Python 侧调用开销 < 1μs
- **低延迟要求**:不会因为 span export 阻塞 GIL
- **容器/Serverless**:单文件部署,冷启动快,无 grpcio 编译依赖
- **阿里云全栈**:traces → ARMS,metrics → ARMS,logs → SLS 任意 logstore,一个 `Config` 搞定

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

Each signal is configured via a **Sink** object. A signal is enabled if its Sink is present in the `sinks` list.

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
| `OtlpLogSink` | Any OTLP collector | gRPC / HTTP | Logs via OTLP (falls into trace instance's `-logs` logstore on SLS) |
| `SlsLogSink` | Aliyun SLS native API | HTTPS | Logs to **any** SLS logstore (not limited to trace instance) |

## Console-only mode

```python
# No sinks → no network, just terminal output
ptx.init(ptx.Config(service_name="my-app", console_level="debug"))
```

## Context propagation (server-side)

```python
# with-syntax auto-restores context on exit
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

MIT
