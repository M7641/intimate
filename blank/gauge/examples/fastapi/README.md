# Example: measuring a FastAPI app with gauge

Shows gauge pointed at a real [FastAPI](https://fastapi.tiangolo.com/) app,
driven by its **OpenAPI spec** rather than a single hard-coded URL. FastAPI
publishes `/openapi.json` automatically, so gauge can discover and exercise every
documented GET endpoint.

`app.py` exposes a spread of endpoint shapes gauge's OpenAPI reader handles:

- `GET /health` — plain
- `GET /items/{item_id}` — path parameter (filled with a sample value)
- `GET /search?q=...` — required query parameter (filled with an example)
- `GET /work` — real CPU + memory work, so the load has a measurable footprint

## Run

Everything is in `gauge.toml`, so it is one command (build included):

```sh
uv run gauge run --config examples/fastapi/gauge.toml
```

gauge builds the image, waits for `/health`, fetches `/openapi.json`, hammers the
four GET endpoints round-robin, and writes a report:

```
       peak memory  163.5 MB
          CPU time  7.499 CPU·s
  mem ▂▃▄▅▆▇█  44→164 MB
OpenAPI: exercising 4 GET endpoint(s)
report written to examples/fastapi/report/ (run.json, memory.png, cpu.png)
```

## What it teaches

- **OpenAPI-driven load** covers a whole API's surface with no per-endpoint
  wiring — point gauge at the spec and it finds the routes.
- The **~44 MB baseline** is much lighter than the Dash example's ~74 MB:
  FastAPI + uvicorn + pydantic import less than dash + plotly + flask. Comparing
  baselines across frameworks is exactly the kind of capacity question gauge
  answers.
- The **report folder** (`report/memory.png`, `report/cpu.png`) plots the same
  timeline the terminal sparklines summarize.

## Multiple workers

A production FastAPI deployment runs several uvicorn workers. The app reads a
`WORKERS` env var, so you can measure that — set it via `run_args`:

```sh
uv run gauge run --config examples/fastapi/gauge.toml --run-arg=-e --run-arg=WORKERS=4
```

With 4 workers the baseline jumps from ~44 MB to ~220 MB (each worker is a
process importing FastAPI) while throughput roughly quadruples — gauge sums every
worker automatically, because cgroup accounting covers the whole container.

Note: uvicorn needs the app as an **import string** (`"app:app"`) to fork
workers — passing the app object instead makes it exit immediately with
`You must pass the application as an import string`. `app.py` does this correctly.

## What's actually in RAM (Python-specific)

gauge tells you *how much* memory the container uses. To see *what* that memory
is — which source lines hold the resident bytes — run the introspection helper:

```sh
uv run python examples/fastapi/introspect.py
```

It runs the app with Python's `tracemalloc` on (via `GAUGE_TRACEMALLOC=1`), drives
some `/work` load, then queries the app's `/debug/memory` endpoint and writes
`report/whats_in_ram.png`. A sample result:

```
RSS 265.3 MB  ·  tracemalloc accounts for 75.22 MB (peak 75.46 MB)

  size MB     count  where
   48.841       402  app.py:52                              <- the /work cache
   12.510    104323  <frozen importlib._bootstrap_external>:511
    1.939     18442  <frozen importlib._bootstrap>:491
    ...
```

The one line that appends to `_cache` in `/work` dominates — that is where the
memory growth lives. Everything else is Python's import machinery (the framework
baseline).

Two honest caveats this makes visible:

- **`tracemalloc` inflates RSS.** Storing a traceback per allocation is why RSS
  here (265 MB) is far above a normal single-worker run (~44 MB). Use this to see
  *where*, and `gauge run` for the true total.
- **Traced (75 MB) ≪ RSS (265 MB).** `tracemalloc` only sees *Python* allocations.
  Native memory (pydantic-core, uvicorn's C bits, allocator overhead) is invisible
  to it. To attribute *that*, use `memray` — see `../../docs/python-approaches.md`.

This lives in the example, not in gauge, because allocation attribution is
language-specific — the same idea for a Rust or Go app would use entirely
different tools.

## Going deeper: memray (native allocations + flamegraph)

`tracemalloc` above misses native memory. [memray](https://bloomberg.github.io/memray/)
does not — it intercepts `malloc`, so C-extension and allocator memory is counted
too, and it produces an interactive flamegraph. A second helper runs the app under
memray, drives load, stops it gracefully so the capture finalizes, and renders the
report:

```sh
uv run python examples/fastapi/memray_introspect.py
# then open examples/fastapi/report/memray/flamegraph.html
```

Its terminal summary (peak memory by call site):

```
 Location              Total Memory   %      Own Memory   %      Count
 work at /app/app.py   51.26 MB       73.3%  51.26 MB     73.3%  204
 <module> at app.py    15.02 MB       21.4%  …                   11329
```

Two things this shows that tracemalloc didn't:

- **Honest total.** memray reports a ~70 MB peak — close to the real heap —
  whereas tracemalloc's own bookkeeping inflated RSS to 265 MB. memray is both
  more complete *and* less intrusive.
- **Native memory is counted.** memray intercepts `malloc`, so memory allocated
  by pydantic-core and other C code is attributed to the Python call site that
  triggered it — invisible to tracemalloc. (Add `--native` to the `memray run` in
  `Dockerfile.memray` for C/C++ stack frames too; that needs debug symbols in the
  image to read well, so it is off by default.)

Rule of thumb: reach for `tracemalloc` for a zero-install, Python-only first look;
reach for `memray` when you need the native picture or a flamegraph.

## Notes

- Only GET endpoints are exercised; endpoints requiring a request body are
  skipped (driving those realistically is out of scope for a load probe).
- To measure under a memory cap, uncomment `memory` in `gauge.toml` (or pass
  `--memory 256m` on the command line) and watch `stopped` change to `OOM-killed`.
- `/debug/memory` is excluded from the OpenAPI schema, so `gauge run` does not
  hit it during a normal OpenAPI-driven load.
