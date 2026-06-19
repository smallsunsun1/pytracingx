# Benchmarks

Microbenchmarks comparing pytracingx against `opentelemetry-python` on the
hot path. Two scenarios:

- **`bench_hot_path.py`** — single-thread per-call latency (`start_span`,
  `counter.add`, `logger.emit`)
- **`bench_concurrent.py`** — 8-thread aggregate throughput, simulating a
  high-QPS service or inference engine where the SDK is hammered concurrently

## Run

```bash
pip install pytest-benchmark opentelemetry-sdk opentelemetry-api
pytest benchmarks/ --benchmark-only --benchmark-columns=mean,median,stddev,ops
```

## What it measures

Each benchmark wraps the target call(s) in `pytest-benchmark`'s timer and
reports operations-per-second. Both implementations are configured with a
**noop exporter** pointed at `127.0.0.1:1` so no real network I/O happens —
the numbers isolate **SDK overhead in Python**.

## Results — single thread (Apple M-series)

| Operation | pytracingx | opentelemetry-python | Speedup |
|-----------|-----------|---------------------|---------|
| `start_span` | ~1.8μs | ~31μs | **~17x** |
| `counter.add` | ~0.32μs | ~2.6μs | **~8x** |
| `logger.emit` | ~0.63μs | ~14μs | **~22x** |

## Results — 8 concurrent threads (Apple M-series)

8 threads × 500 ops each (4000 total ops measured as one batch):

| Operation | pytracingx | opentelemetry-python | Speedup |
|-----------|-----------|---------------------|---------|
| 8t × `start_span` | ~6.9ms (≈1.7μs/op) | ~139ms (≈34.7μs/op) | **~20x** |
| 8t × `counter.add` | ~1.8ms (≈0.44μs/op) | ~10.8ms (≈2.7μs/op) | **~6x** |

### Why concurrent throughput matters

Python's GIL means even with N threads, only one thread runs Python bytecode
at a time. SDKs that hold the GIL during span construction / serialization
become a **global throughput bottleneck**.

**pytracingx releases the GIL** after argument conversion, so:

- The Rust-side work (allocation, attribute conversion, batch enqueue) runs
  on Rust threads truly in parallel
- Python application code on other threads is **never blocked** by export work
- Per-op cost stays flat from 1 to N threads (1.8μs → 1.7μs)

`opentelemetry-python` holds the GIL through the whole `start_span` path:

- 8 threads contending for the GIL → per-op cost stays at ~33μs but threads
  serialize → effective throughput is roughly the same as single-thread
- In an inference server with continuous batching, this directly turns into
  **GPU bubble** while Python threads queue up to call into the SDK

The single-thread gap is ~17x; under contention the gap **widens to ~20x for
spans** because pytracingx scales linearly with thread count while OTel does
not. The difference would grow further on machines with more cores.

## Caveats

- These are microbenchmarks. End-to-end throughput depends on batching,
  network, and the backend.
- The OTel Python comparison uses noop exporters, not the real OTLP pipeline.
  In practice the gap widens further when batching + protobuf encoding
  participate, since pytracingx does both off the GIL.
- Results vary by ±2x with hardware, but the **shape** (pytracingx scales,
  OTel-Python doesn't) is consistent across machines.
