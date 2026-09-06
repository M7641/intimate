# gauge

Measure the **exact** memory + CPU footprint of a web app under load.

A web app's resource profile is a function of load, not a single number. `gauge`
runs your app in a container, drives traffic at it, and reads the kernel's own
**cgroup accounting** — `memory.peak` (an exact high-watermark) and `cpu.stat`
(exact CPU microseconds). Those are the counters the OOM-killer and the
scheduler actually use, so the numbers are the real ones, isolated from
everything else on the machine — not "the largest value we happened to sample."

Language-agnostic: it only needs a built image. Runtime-agnostic: works with
**docker or podman**, auto-detected.

## Setup

```sh
uv sync              # install gauge
uv run gauge setup   # install/start a container runtime (podman)
```

`gauge setup` is idempotent — if docker or podman already works it does nothing;
otherwise it installs podman (via Homebrew on macOS) and starts a machine.

This installs one console script, `gauge`, with four subcommands: `gauge setup`
(bootstrap the runtime), `gauge run` (measure), `gauge init` (write a starter
config), and `gauge load` (a standalone HTTP load driver).

## Use — config first

Rather than a long command line, describe the app once in a TOML file and point
gauge at it. Bootstrap one with `gauge init`:

```sh
uv run gauge init my-image --port 8080 --build . --openapi -o app.toml
# edit app.toml, then:
uv run gauge run --config app.toml
```

`gauge` builds the image (if configured), waits for the readiness URL, drives
load, samples cgroup counters throughout, and prints exact figures:

```
                        Gauge
             image  my-image
           runtime  podman
          duration  9.6 s
       peak memory  556.7 MB
       mean memory  315.0 MB
       memory·time  2955.3 MB·s
          CPU time  26.021 CPU·s
 peak / mean cores  2.81 / 2.71
           stopped  load finished
  mem ▁▂▃▃▄▄▅▅▆▆▇█  74→556 MB
  cpu ▁▇▇▇▇▇▇█▇▇▇▇  0.0→2.8 cores
```

Every config value has a matching CLI flag that overrides it, so you can measure
without a file too, or tweak one setting: `gauge run --config app.toml --memory 400m`.

See `AGENTS.md` for the full config schema and a task-oriented operating guide.

## Load strategies

Pick one in the `[load]` section (or via flags):

- **Single URL** (`url` / `--load-url`) — hammer one endpoint.
- **OpenAPI** (`openapi_url` / `--openapi-url`) — gauge fetches the spec and
  exercises every documented **GET** endpoint round-robin, filling path and
  required query params with sample values. Covers a whole API with no wiring.
- **Custom command** (`command` / `--load "<cmd>"`) — run any external tool
  (wrk, k6, hey).

## Output — plots + data

Set `[output] dir` (or `--out-dir report/`) and gauge writes a folder with
`run.json` (summary + full timeline) and `memory.png` / `cpu.png` line charts —
the same timeline the terminal sparklines summarize.

## Behaviour under a cap

`--memory 400m` / `--cpus 1.5` (or the `[container]` keys) apply a **real cgroup
limit**. If the container is **OOM-killed**, gauge reports it cleanly (in red)
instead of crashing — a useful way to find an app's true floor.

## Examples

- `examples/fastapi/` — Python; config-driven, **OpenAPI**-derived load; plus
  Python heap lenses (`tracemalloc`, `memray`) for _what's_ in RAM.
- `examples/dash/` — Python; single-endpoint load; shows a heavy import baseline.
- `examples/axum/` — Rust (Axum); ~2 MB baseline vs Python's ~46 MB, with a
  `dhat` heap-profiling lens.
- `examples/rama/` — Rust (Rama); a second Rust framework confirming the tiny
  baseline. Same endpoints across all four, measured on one scale.

## Layout

Each module is a folder holding its source and co-located tests side by side
(e.g. `runner/runner.py` + `runner/runner_test.py`), with an `__init__.py`
re-exporting the public API so imports stay `from gauge.runner import ...`.

- `src/gauge/runner/` — core: runtime detection, cgroup reads, the sample loop.
- `src/gauge/cli/` — the Typer front-end (`gauge setup` / `run` / `init` / `load`).
- `src/gauge/setup/` — bootstrap a container runtime (install/start podman).
- `src/gauge/config/` — the `RunConfig` and TOML loader / `init` template.
- `src/gauge/openapi/` — derive GET requests from an OpenAPI spec.
- `src/gauge/load/` — the concurrent HTTP load driver.
- `src/gauge/report/` — write the run.json + PNG plots folder.

## Notes and honest limits

- On macOS both docker and podman run Linux inside a VM, so the numbers include
  that VM's overhead. This is honest — it is what you would actually deploy — but
  it is not bare-metal.
- `memory.peak` and `cpu.stat` are cgroup **v2** (Docker Desktop, Podman machine,
  modern Linux); gauge falls back to v1 paths if v2 is absent.
- This measures _how much_, not _which code_. To attribute memory/CPU to specific
  lines, reach for a profiler — see `docs/python-approaches.md` for the Python
  landscape (tracemalloc, memray, Scalene, py-spy) and how it composes with gauge.
- gauge is for a **service under load**. To size a one-shot **workflow** (a batch
  job / script / CLI) instead — `/usr/bin/time` for cores + wall time, memray or
  dhat for the heap — see `docs/workflow-tracking.md`.
