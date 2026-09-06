# Example: measuring a Rama (Rust) app with gauge

[Rama](https://ramaproxy.org/) is a modular Rust service framework — the same one
used to build proxies — with an HTTP server assembled from composable tower-style
services. This example gives it the same endpoints as the Axum and FastAPI ones,
so gauge measures all three on one scale.

## Run

```sh
uv run gauge run --config examples/rama/gauge.toml
```

## What it teaches

### A second Rust baseline, far below Python

Measured the same way (idle RSS, after `/health`, averaged over a few reads):

| App | Framework | Idle baseline |
|---|---|---|
| `examples/rama` | Rama (Rust) | **~2 MB** |
| `examples/axum` | Axum (Rust) | **~2 MB** |
| `examples/fastapi` | FastAPI (Python) | ~46 MB |
| `examples/dash` | Dash (Python) | ~66 MB |

The headline is the **language gap**: both Rust frameworks rest at ~2 MB, roughly
30× lighter than the Python frameworks. For a dense fleet of small services, that
per-instance floor is often the number that decides how many fit on a host — and
gauge measures it directly, the same way, across all of them.

### Honest comparison: Rama vs Axum

At rest and under this toy's load, Rama and Axum are **the same size** (~2 MB idle,
and an identical ~370 MB peak — both dominated by the same `/work` cache). Rama's
reputation for leanness is about *connection-heavy* workloads — thousands of
concurrent proxied streams, where per-connection state dominates — which this
three-endpoint example does not exercise. The honest takeaway here is "Rust ≪
Python," not "Rama < Axum"; measure your own workload before claiming a framework
wins on memory.

## What's actually in RAM (dhat)

The Rust heap-profiling lens is the same as the Axum example's — see
`../axum/README.md` and `../axum/dhat_introspect.py`. To profile this app, add the
same `dhat` dependency, feature, and global allocator to it (Rama's graceful
shutdown lets the profiler flush on stop).

## Notes

- Rama's `http-full` feature pulls a C toolchain (for BoringSSL), so the builder
  stage installs cmake/clang/perl/go — the runtime image stays slim.
- Like Axum, `/work` touches every page of its 256 KB chunk so the memory is
  resident, not a lazy zero page (see the Axum README).
- Uses `rama` `0.3.0-alpha.4`; the API is still pre-1.0 and may shift.
