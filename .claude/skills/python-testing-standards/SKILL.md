---
name: python-testing-standards
description: >-
  Our Python testing standards (uv + pytest + moon) — the five kinds of tests we
  run (unit, benchmark/perf-gate, property-based, mutation, fuzz), which tool to
  use for each, the Rust-style co-located file layout, and how each wires into a
  moon + proto monorepo's tasks + CI. Use this whenever adding or improving Python
  tests, setting up a benchmark or perf gate, wiring a new test task into moon/CI,
  or deciding which kind of test fits a piece of Python code. Also covers
  integration tests that need a real backing service: spinning up and seeding a
  disposable Postgres container, or mocking S3 with an S3Mock container, then
  throwing it away. Trigger even when the user just says "add tests", "benchmark
  this", "fuzz it", "property test", "catch regressions", "make this defensible",
  "seed test data", "test against Postgres", "mock S3", "spin up a test database",
  or "testcontainers" in a Python project, without naming a specific tool. For
  Rust, use rust-testing-standards instead.
---

# Python testing standards (uv + pytest + moon)

We run **five kinds of tests**. Each answers a different question, runs on a
different cadence, and gates differently. The goal is that "optimised" and "works
well" become *numbers a CI gate defends*, not vibes.

## The principles behind it

Four convictions shape everything below:

- **"Optimised" must be a number, not a vibe.** If there's no benchmark, you can't
  defend "works well" or catch a regression you can't measure. The bar is a
  benchmark on each hot path with a CI gate that fails past an *X%* regression —
  a perf *budget*, not a perf *hope*.
- **Five kinds, same set as Rust.** Normal (unit/integration), benchmark,
  property, mutation, and fuzz — so a service's defensibility doesn't depend on
  what it's written in. (The Rust equivalents live in `rust-testing-standards`.)
- **Tests live next to the code.** Python should feel like Rust: each feature is a
  folder whose source, tests and benchmark sit side by side, with an `__init__.py`
  keeping imports unchanged.
- **Land it incrementally.** Prototype the pattern on one project, prove it, then
  roll it out to the hot path of every service.

## The five kinds

| Kind | Question it answers | Tool | CI cadence |
| --- | --- | --- | --- |
| **Unit / integration** | Does it do the right thing? | `pytest` | every PR (blocking) |
| **Benchmark + perf gate** | Is it still fast? | `pytest-benchmark` + gate | every PR (blocking) |
| **Property-based** | Does it hold for *all* inputs? | `hypothesis` | every PR (blocking) |
| **Mutation** | Are the tests actually testing? | `mutmut` | nightly / pre-merge (ratchet) |
| **Fuzz** | Does untrusted input crash it? | `atheris` | nightly (time-boxed) |

Tools live as dev-dependencies in the workspace `pyproject.toml`; pytest runs via
`uv run --no-sync pytest` from the project dir (avoids `sys.path` shadowing between
members).

Coverage (`pytest-cov`) is complementary, not one of the five — see
`references/coverage.md`. Integration tests that need a real Postgres or a mock S3
have their own pattern — see `references/integration.md`. The reusable perf-gate
comparator is `templates/bench_gate.py`; the disposable-Postgres helper is
`templates/pg_harness.py`.

## File layout — tests next to the code (Rust-style)

Each feature is a **folder** under `src/<pkg>/`, holding its source, tests and
benchmark side by side, with an `__init__.py` re-exporting the public API so
imports are unchanged:

```
src/mypkg/
├── __init__.py            # public API: `from mypkg import Engine`
├── geo/                   # a feature
│   ├── __init__.py        # re-exports → `from mypkg.geo.geo import distance_matrix`
│   ├── geo.py             # source
│   ├── geo_test.py        # unit + property tests, next to the code
│   └── geo_bench.py       # benchmarks (hot path)
└── postcode/  (postcode.py · postcode_test.py · postcode_bench.py)
```

Configure collection so tests are found but benchmarks are excluded from the
normal run (they're opt-in via the `bench` task):

```toml
# pyproject.toml
[tool.pytest.ini_options]
addopts = ["--import-mode=importlib"]        # path-based import; no sys.path games
python_files = ["test_*.py", "*_test.py"]    # NOT *_bench.py
```

`--import-mode=importlib` matters: it imports test files by path under unique
names, so duplicate basenames across feature folders don't collide the way they do
with the legacy prepend mode.

## 1. Unit / integration — pytest

The inherited `test` task runs `uv run --no-sync pytest -q`. Keep network/IO out of
unit tests — mark them and select them separately, or they make the suite slow and
flaky:

```python
# geo_test.py
from mypkg.geo import distance_matrix

def test_matrix_is_square_with_zero_diagonal():
    m = distance_matrix([(53.0, -2.0), (53.5, -2.5)])
    assert len(m) == 2 and len(m[0]) == 2
    assert m[0][0] == 0 and m[1][1] == 0
```

## 2. Property-based — Hypothesis

Property tests are just pytest tests using `@given`; they live in the same
`<feature>_test.py`. They find the edge cases example tests miss. Dev-dep:
`hypothesis`.

```python
from hypothesis import given, strategies as st
from mypkg.postcode import clean_postcode

@given(st.text())
def test_clean_postcode_is_idempotent(s):
    once = clean_postcode(s)
    assert clean_postcode(once) == once   # cleaning twice == cleaning once
```

Keep example counts bounded (Hypothesis defaults are fine) so it stays a PR gate.

## 3. Benchmark + perf gate — pytest-benchmark

Benchmarks live in `<feature>_bench.py` using the `benchmark` fixture; a committed
baseline is the budget; a small comparator gates on regression. Dev-dep:
`pytest-benchmark`.

```python
# geo_bench.py
from mypkg.geo import distance_matrix

def _locations(n=60):
    return [(53.0 + i*0.01, -2.0 - i*0.01) for i in range(n)]

def test_bench_distance_matrix(benchmark):
    m = benchmark(distance_matrix, _locations())
    assert len(m) == 60
```

The gate uses a **custom JSON comparator** (`templates/bench_gate.py`), not
`--benchmark-compare-fail`. Why: pytest-benchmark keys its storage by machine tag
(`Linux-CPython-3.12-…`), so a committed baseline silently fails to match on a
different CI runner and the built-in gate becomes a no-op. The comparator matches
benchmarks **by name** and gates on the **`min`** stat (least noisy). Copy it into
the project root (env: `BENCH_MAX_REGRESSION` default 25, `BENCH_STAT` default
`min`).

moon tasks (in the project's `moon.yml`):

```yaml
tasks:
  bench:            # the gate — runs in CI
    command: 'bash'
    args:
      - '-c'
      - 'uv run --no-sync pytest -o python_files="*_bench.py" --benchmark-only --benchmark-json=.benchmarks/current.json && uv run --no-sync python bench_gate.py .benchmarks/baseline.json .benchmarks/current.json'
    toolchain: 'system'
    deps: ['~:sync']
    options: { cache: false }
  bench-update:     # regenerate the committed baseline (stable host only)
    command: 'uv'
    args: ['run','--no-sync','pytest','-o','python_files=*_bench.py','--benchmark-only','--benchmark-json=.benchmarks/baseline.json']
    toolchain: 'system'
    deps: ['~:sync']
    options: { cache: false, runInCI: false }
```

Commit `.benchmarks/baseline.json`; gitignore `.benchmarks/current.json`. The
baseline is machine-sensitive — regenerate it on (or matched to) the CI runner for
the gate to be meaningful; the threshold absorbs normal cross-run noise.

## 4. Mutation — mutmut

Mutation testing changes the source (flips `<` to `<=`, etc.) and re-runs the
tests; a surviving mutant = a missing assertion. Slow → **not a PR gate**; run it
on a schedule and ratchet the score. Tool: `mutmut` (dev-dep), or `cosmic-ray` for
larger suites.

```yaml
  mutation:
    command: 'uv'
    args: ['run','--no-sync','mutmut','run']
    toolchain: 'system'
    deps: ['~:sync']
    options: { cache: false, runInCI: false }   # invoked by a nightly workflow
```

Workflow: run nightly, read `mutmut results`, turn surviving mutants into new
unit/property tests. Don't fail the build on new survivors; fail only if the
killed-ratio drops below the recorded floor.

## 5. Fuzz — Atheris

Atheris is Google's coverage-guided fuzzer for Python (libFuzzer-backed). Put
targets in a `fuzz/` dir; run time-boxed on a schedule, never per-PR. Tool:
`atheris` (dev-dep; needs a C toolchain to build).

```python
# fuzz/fuzz_clean_postcode.py
import atheris, sys
from mypkg.postcode import clean_postcode

def one(data: bytes):
    clean_postcode(atheris.FuzzedDataProvider(data).ConsumeUnicode(64))

atheris.Setup(sys.argv, one); atheris.Fuzz()
```

Run with a budget: `python fuzz/fuzz_clean_postcode.py -max_total_time=300`. Commit
any crash corpus that reproduces a bug, then add it as a regression unit test.

## How testing wires into a moon + proto monorepo

Three facts about the toolchain shape every decision here:

1. **moon is the task runner and the CI gate.** CI runs `moon ci`, which diffs
   against the base branch and runs the *affected* projects' tasks. Language-wide
   task defaults live in `.moon/tasks/python.yml` (`inheritedBy: language`).
   Project-specific tasks (like a `bench` gate) go in that project's `moon.yml`
   under `tasks:`. A task runs in CI unless it sets `options.runInCI: false`.

2. **proto pins every tool.** Versions live in `.prototools`. Built-in tools
   (python/uv) need no plugin; a standalone CLI binary needs a **vendored plugin**
   under `proto-plugins/*.toml` with a `file://` locator — a reviewed pointer to
   the tool's official releases, not a community plugin fetched at runtime.
   Library-level tools (`pytest-benchmark`, `hypothesis`, `mutmut`, `atheris`) are
   NOT proto tools — they're dev-dependencies in `pyproject.toml`
   (`[dependency-groups] dev = [...]`); `uv sync` picks them up. Respect the
   supply-chain cooldown (`exclude-newer`).

3. **The hooks mirror CI.** `lefthook` runs `moon run :lint`/`:test --affected` on
   pre-push, so fast tests stay green locally before they ever reach CI.

## What gates a PR vs what runs nightly

This is the most important judgement call — get it wrong and CI is either useless
or unbearable.

**Block the PR** with tests that are *fast and deterministic*: unit, property
(bounded examples), and the benchmark **regression gate** (compare to a committed
baseline, fail past a threshold). These give a yes/no answer in seconds.

**Run nightly / scheduled** (a cron workflow, time-boxed) for tests that are
*slow or open-ended*: mutation and fuzz. They explore an unbounded space, so they
can't give a clean per-PR verdict. Treat their output as a **ratchet**: track the
score, fail only if it drops below the last recorded floor — never on "found
something new after an hour". Surface what was skipped; silent truncation reads as
"covered everything" when it wasn't.

Why this split: a PR gate must be a *budget*, not a *hope*. A test that sometimes
takes 2 seconds and sometimes 40 minutes can't be a gate.

## Choosing the right kind for a piece of code

- **Pure function with a clear contract** (parsing, distance maths, serialisation)
  → property test first (it finds the edge cases you'd miss), plus a couple of
  example-based unit tests for readability.
- **Hot path** (anything you'd call "optimised") → a benchmark with a committed
  baseline and a regression gate. No benchmark = the perf claim is undefendable.
- **Parser / decoder / anything eating untrusted bytes** → fuzz target.
- **Existing test suite you don't trust** → run mutation testing once; every
  surviving mutant is a missing assertion. Fix by adding tests, then re-run.
- **I/O / network boundary** → keep it a thin, separately-marked integration test;
  don't put network in the unit or benchmark path (it makes both flaky and slow).
- **Code whose behaviour depends on a real backing service** (server-side SQL, a
  query over seeded fixtures, S3 object operations) → an integration test against
  a disposable, seeded Postgres or an S3Mock container. See
  `references/integration.md`. Don't fake Postgres with SQLite if the production
  target is Postgres.

## Rolling this out to a project

1. Identify the **hot path(s)** and the **pure functions** — those drive the
   benchmark and property tests respectively.
2. Add the dev-dependency (library tools) or the proto vendored plugin (CLI tools).
3. Add/confirm the moon tasks: `test` (inherited), plus `bench` (+ `bench-update`)
   for the perf gate; mutation/fuzz as `runInCI: false` tasks invoked by a
   scheduled workflow.
4. Commit the benchmark **baseline** so the gate has something to compare against.
5. Make `moon ci` a required status check so the gates actually block merges.
