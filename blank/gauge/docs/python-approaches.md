# Measuring resource requirements in Python — the landscape

`gauge` samples the RSS and CPU of a process tree from the _outside_. That is
one deliberate choice on two axes, and it is worth seeing the whole space before
committing to it. This document maps the alternatives for **Python** web apps
and says when each beats a black-box sampler.

## First, two questions people conflate

Before picking a tool, decide which question you are actually asking. Most
confusion comes from a tool answering a different one than you meant.

**Memory: resident vs allocated.** _RSS_ (resident set size) is the physical
memory the OS currently gives the process — what `gauge`, `time`, and `docker
stats` report. _Heap allocations_ are every `malloc`/Python object your code
requested, whether or not it is still resident. RSS tells you "will this get
OOM-killed"; allocation tracking tells you "which line grew the heap." They
diverge: freed memory can stay resident (the allocator holds it), and a leak
shows in allocations before RSS notices.

**CPU: time vs utilization.** _CPU time_ (user+system seconds) is total work
done — good for "how expensive is one request." _Utilization_ (%, possibly

> 100% across cores) is instantaneous pressure — good for "how many cores must I
> provision." A batch job cares about the first; a web app under concurrency cares
> about the second.

## Second, two axes that separate every tool

- **External vs in-process.** External tools observe from outside (no code
  change, language-agnostic, coarse). In-process tools instrument the runtime
  (precise, Python-aware, require importing/wrapping and add overhead).
- **Sampling vs instrumentation.** Samplers peek periodically (cheap, miss
  sub-interval spikes, statistical). Instrumentation records every event
  (exact, heavy, can distort timing).

`gauge` sits at **external + sampling** — the cheapest, most universal corner,
and the least precise. Everything below trades some universality for detail.

## The tools, by category

### A. External, language-agnostic (gauge's neighbourhood)

| Tool                                          | Measures                          | Notes                                                                                         |
| --------------------------------------------- | --------------------------------- | --------------------------------------------------------------------------------------------- |
| `/usr/bin/time -l` (macOS) / `-v` (Linux)     | peak RSS, CPU time                | one final number, zero setup, no timeline                                                     |
| external RSS sampling (psutil)                | RSS + CPU% over time, whole tree  | approximate, needs only a launchable command; gauge started here before committing to cgroups |
| **gauge** — cgroup `memory.peak` + `cpu.stat` | **exact** memory + CPU, isolated  | needs the app containerized; the counters the kernel itself uses                              |
| `resource.getrusage()` (stdlib)               | peak RSS, CPU time, from _inside_ | one line, but self-measurement only — the process reports on itself                           |

These treat the app as a black box. They cannot tell you _which code_ is
responsible — only _how much_ in total. For capacity planning that is often all
you need — and **gauge deliberately went all-in on cgroups**: exact kernel
accounting beats sampling for the one question this tool answers.

### B. Heap-allocation profilers (which code grew memory)

These answer "what allocated this," not just "how much is resident."

- **`tracemalloc`** (stdlib, since 3.4). Tracks Python-level allocations and
  attributes them to the source line. Take two snapshots, diff them, see exactly
  what grew between two points. Zero install, Python-only allocations (misses C
  extensions' native `malloc`), moderate overhead. The default first reach for a
  suspected leak.
- **`memray`** (Bloomberg). The heavyweight. Traces _native_ allocations too, so
  it sees NumPy/pandas/C-extension memory that `tracemalloc` misses. Produces
  flamegraphs, live mode, and a pytest plugin to gate memory in CI. Linux/macOS.
  When you need to know where memory really went, this is the answer.
- **`Fil`** (Itamar Turner-Trauring). Built for data-science batch jobs:
  reports the single moment of **peak** memory and the full allocation call tree
  at that instant. Narrow, but perfect for "why did this pipeline OOM."

### C. Line-level and combined profilers

- **`Scalene`** (UMass). The most complete single tool: CPU **and** memory
  **and** GPU, at line granularity, and — uniquely — it separates time/memory
  spent in _Python_ from _native_ code. Low overhead via sampling. If you want
  one white-box tool for a Python service, start here.
- **`memory_profiler`**. Classic line-by-line RSS (`@profile` decorator). Simple
  mental model, but largely unmaintained and higher overhead — Scalene or memray
  supersede it for new work.

### D. Sampling CPU profilers (where time goes, no code change)

- **`py-spy`** (Ben Frederickson). Samples the call stack of a _running_ Python
  process from outside — you can `py-spy top --pid <pid>` on a live production
  server without restarting or importing anything. Flamegraphs via `record`.
  This is the CPU counterpart to what `gauge` does for memory: external,
  attach-anytime, safe under load.
- **`Austin`**. Same idea, a tiny C frame-stack sampler; pairs with many
  visualizers. Very low overhead.
- **`Pyinstrument`**. Statistical profiler that renders a readable call tree —
  great for "why is this request slow" during development.

### E. Deterministic CPU profilers (exact call counts)

- **`cProfile`** (stdlib) / **`yappi`**. Record every call — exact counts and
  cumulative time, but the instrumentation overhead distorts absolute timings
  and they are awkward under concurrency (`yappi` handles threads/async better).
  Use for algorithmic hotspots in a single run, not for load characterization.

## Choosing — a short decision guide

- **"How much RAM/CPU to provision for this service?"** → external black-box.
  `gauge` for a quick timeline on any command; **cgroups/`docker stats`** when
  you need the number to be exact and reproducible.
- **"It leaks — which code?"** → **`tracemalloc`** first (free, Python-level);
  **`memray`** if the growth is in native/C-extension memory.
- **"Why did this batch job OOM?"** → **`Fil`** (peak-memory call tree).
- **"One tool for CPU + memory, line by line, in dev"** → **`Scalene`**.
- **"A live production process is hot/leaking — inspect without restarting"** →
  **`py-spy`** (CPU) alongside `gauge`/cgroups (memory), both attach-safe.
- **"Where does one request spend its time?"** → **`Pyinstrument`** (readable)
  or **`cProfile`**.

## How this positions gauge

`gauge` is intentionally the **external, exact, load-aware** corner: containerize
the app, drive it, and read the kernel's own cgroup counters — no code changes,
no language assumptions, and numbers exact enough to size a deployment. Its blind
spot — _which code_ is responsible — is exactly what the white-box tools in
sections B–E illuminate.

They compose. A realistic investigation runs `gauge` first to see _that_ an app
is heavy, by _how much_, and _when_ it climbs, then attaches `memray`, `Scalene`,
or `py-spy` inside the same container to see _why_. gauge tells you the shape and
size of the problem; the profilers tell you the cause.
