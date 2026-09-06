# AGENTS.md — using gauge

gauge measures the **exact** memory + CPU footprint of a containerized web app
under load, by reading the kernel's cgroup accounting (`memory.peak`,
`cpu.stat`). This file is the operating manual for an agent driving the tool.

## Prerequisites

- `uv sync` (core) and `uv sync --group examples` (to run the bundled examples).
- A container runtime must be **running**. Run `uv run gauge setup` — it installs
  podman (via Homebrew on macOS) and starts a podman machine if nothing is
  already working. It is idempotent and no-ops when docker/podman already answer.
  gauge auto-detects the runtime; force one with `--runtime`.
- Run everything through uv: `uv run gauge ...`.

## The normal workflow

Prefer a config file over a long command line:

1. `uv run gauge init <image> --port <p> [--build <ctx>] [--openapi] -o app.toml`
   writes a starter config.
2. Edit `app.toml` (see schema below).
3. `uv run gauge run --config app.toml`.

CLI flags override config values, so you can tweak without editing the file:
`uv run gauge run --config app.toml --memory 400m`.

## Commands

- `gauge setup [--runtime podman|docker] [--yes]` — install/start a container
  runtime. Run once before anything else.
- `gauge run [IMAGE] [--config app.toml] [overrides…]` — build (optional), launch,
  drive load, print exact figures, optionally write a report folder.
- `gauge init [IMAGE] [--port] [--build] [--openapi] [-o]` — write a starter config.
- `gauge load URL [-n N] [-c C]` — standalone HTTP load driver (rarely needed
  directly; `gauge run` drives load itself).

Run `uv run gauge run --help` for every flag.

## Config schema (TOML)

```toml
image = "my-image"              # required (or pass as the run argument)

[build]                         # optional — build the image before measuring
context = "path/to/app"
# dockerfile = "path/to/Dockerfile"

[container]
port = 8080                     # port the app listens on INSIDE the container
# host_port = 8080              # defaults to port
# memory = "512m"              # real cgroup cap; OOM is reported, not crashed
# cpus = "2"
# run_args = ["-e", "KEY=val"] # raw extra `run` flags

[ready]
url = "http://127.0.0.1:8080/health"   # polled before load starts
timeout = 30

[load]                          # pick ONE of the three
url = "http://127.0.0.1:8080/work"          # a) single endpoint
# openapi_url = "http://127.0.0.1:8080/openapi.json"  # b) derive GETs from OpenAPI
# command = "wrk -t4 -c16 -d10s http://..."           # c) custom load tool
requests = 2000
concurrency = 16

[output]
dir = "report"                  # writes run.json + memory.png + cpu.png
interval = 0.3                  # cgroup sampling period (seconds)
```

## Load strategies

- **Single URL** (`url`): hammer one endpoint. Best when one route dominates.
- **OpenAPI** (`openapi_url`): gauge fetches the spec and exercises every
  documented **GET** endpoint round-robin, filling path params with a sample
  value and required query params with an example. Endpoints needing a request
  body are skipped. Best for measuring a whole API's surface.
- **Custom command** (`command`): run any external load tool (wrk, k6, hey).

## Output — what to read

Terminal shows a table + memory/CPU sparklines. Key fields:

- `exact_peak_mem_mb` — the number to size memory limits by (OOM ceiling).
- `exact_cpu_seconds` — total CPU work; `mean_cpu_cores` sizes CPU requests.
- `memory·time` (MB·s) — integral; sizes _cost_ better than the peak alone.
- `stopped` — `load finished` (clean), or `OOM-killed` / `exited` (the app died;
  shown in red). If OOM-killed under a `--memory` cap, the app needs more than
  the cap.

With `[output] dir` set, the same data is written as `run.json` plus
`memory.png` / `cpu.png` line charts.

## Gotchas

- **The app must bind `0.0.0.0` inside the container**, not `127.0.0.1`, or the
  published port can't reach it. The example Dockerfiles set `GAUGE_HOST=0.0.0.0`.
- **`ready.url` must actually return 200**, or load never starts and the run
  stalls until `ready.timeout`.
- **If the container crashes on startup, gauge prints its last logs** and stops —
  read them. A common cause: launching a multi-worker server wrong. uvicorn/
  gunicorn need the app as an **import string** (`"app:app"`), not the app object,
  to fork workers; passing the object makes uvicorn exit immediately. Set worker
  count via env (`--run-arg=-e --run-arg=WORKERS=4`); the cgroup sums all workers.
- On macOS the numbers include the runtime VM's overhead — honest (it's what you
  deploy) but not bare-metal.
- gauge measures _how much_, not _which code_. To attribute usage to source
  lines, use a profiler — see `docs/python-approaches.md`.

## Worked examples

Same endpoints across all four, so they compare on one scale (Rust ~2 MB idle vs
Python ~46–66 MB). Each carries a language-specific "what's in RAM" lens.

- `examples/fastapi/` — Python; OpenAPI-derived load; `tracemalloc` + `memray` lenses:
  `uv run gauge run --config examples/fastapi/gauge.toml`
- `examples/dash/` — Python; single-endpoint load, heavy import baseline:
  `uv run gauge run --config examples/dash/gauge.toml`
- `examples/axum/` — Rust (Axum); `dhat` heap lens:
  `uv run gauge run --config examples/axum/gauge.toml`
- `examples/rama/` — Rust (Rama); a second lean Rust baseline:
  `uv run gauge run --config examples/rama/gauge.toml`
