# Example: measuring an Axum (Rust) app with gauge

The Rust counterpart to the FastAPI example — same endpoints (`/health`,
`/items/{id}`, `/search`, `/work`), so it is measured the same way. The reason to
have it is **contrast**: gauge is language-agnostic (it reads cgroup counters, not
Python internals), so it puts a Rust service and a Python service on the same
scale.

## Run

```sh
uv run gauge run --config examples/axum/gauge.toml
```

A multi-stage Dockerfile compiles the binary and copies it into a slim runtime;
gauge then builds, loads, and measures it. A sample result:

```
       peak memory  372.8 MB
       mean memory  233.6 MB
          CPU time  3.567 CPU·s
  mem ▁▂▃▄▅▆▇█  4→373 MB
```

## What it teaches

### Baseline is a fraction of Python's

The idle baseline is **~2 MB** — the Rust binary plus the tokio runtime. Compare
across the examples, same machine, same gauge (idle RSS after `/health`):

| App | Framework | Idle baseline |
|---|---|---|
| `examples/axum` | Axum (Rust) | **~2 MB** |
| `examples/rama` | Rama (Rust) | ~2 MB |
| `examples/fastapi` | FastAPI (Python) | ~46 MB |
| `examples/dash` | Dash (Python) | ~66 MB |

For a fleet of small services, that fixed per-instance floor is often the number
that decides density. gauge measures it directly and comparably. (The two Rust
frameworks are the same size at rest — the gap that matters here is language, not
framework.)

### Allocated ≠ resident

`/work` retains 256 KB per request, but it explicitly **touches every page**:

```rust
let mut chunk = vec![0u8; 256_000];
for i in (0..chunk.len()).step_by(4096) { chunk[i] = 1; }  // commit the pages
```

Without that loop, `vec![0u8; N]` is backed by copy-on-write zero pages — the
memory is *allocated* but never *resident*, so gauge (which reads RSS) would show
almost no growth. That gap is the whole point of measuring resident memory: it is
what is actually in RAM, not what was requested.

## What's actually in RAM (dhat — the Rust lens)

Rust has no tracemalloc or memray, but [dhat](https://docs.rs/dhat) plays the same
role: a global allocator records every allocation and writes a capture you open in
the [DHAT viewer](https://nnethercote.github.io/dh_view/dh_view.html).

```sh
uv run python examples/axum/dhat_introspect.py
# then open examples/axum/report/dhat/dhat-heap.json in the DHAT viewer
```

Its summary after 200 `/work` requests:

```
dhat: At t-gmax: 51,249,326 bytes in 283 blocks     <- peak heap = the /work cache
dhat: At t-end:  51,207,735 bytes in 212 blocks
```

The peak heap is 51 MB in ~283 blocks — the 200 retained 256 KB chunks. The
capture attributes every byte to the Rust source line that allocated it, exactly
like memray does for Python. This is the pattern across the toolkit: gauge gives
the language-agnostic *how much*; each example carries the language-specific lens
for *what*.

## Notes

- Axum publishes no OpenAPI spec by default, so `gauge.toml` uses a single-URL
  load (`/work`). Add [utoipa](https://docs.rs/utoipa) to expose `/openapi.json`,
  then switch the config to `openapi_url` like the FastAPI example.
- dhat needs a graceful shutdown to flush its capture; the app returns on SIGTERM
  (which `podman stop` sends), so the helper stops rather than kills it.
- Rust's Rust-native profiling landscape beyond dhat: `heaptrack` and `valgrind
  massif` (external, Linux), `bytehound`, or jemalloc's `jeprof`.
