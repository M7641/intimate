---
name: rust-testing-standards
description: >-
  Our Rust testing standards (Cargo + moon) — the five kinds of tests we run
  (unit, benchmark/perf-gate, property-based, mutation, fuzz), the use of
  cargo-nextest as the test runner, which tool to use for each, the co-located
  file layout, and how each wires into a moon + proto monorepo's tasks + CI. Use
  this whenever adding or improving Rust tests, choosing/setting up a test runner,
  setting
  up a benchmark or perf gate, wiring a new test task into moon/CI, or deciding
  which kind of test fits a piece of Rust code. Also covers integration tests that
  need a real backing service: spinning up and seeding a disposable Postgres
  container with testcontainers, or mocking S3 with an S3Mock container, then
  throwing it away. Trigger even when the user just says "add tests", "benchmark
  this", "fuzz it", "property test", "catch regressions", "make this defensible",
  "seed test data", "test against Postgres", "mock S3", "spin up a test database",
  or "testcontainers" in a Rust project, without naming a specific tool. For
  Python, use python-testing-standards instead.
---

# Rust testing standards (Cargo + moon)

We run **five kinds of tests**. Each answers a different question, runs on a
different cadence, and gates differently. The goal is that "optimised" and "works
well" become *numbers a CI gate defends*, not vibes.

## The principles behind it

Four convictions shape everything below:

- **"Optimised" must be a number, not a vibe.** If there's no benchmark, you can't
  defend "works well" or catch a regression you can't measure. The bar is a
  benchmark on each hot path with a CI gate that fails past an *X%* regression —
  a perf *budget*, not a perf *hope*.
- **Five kinds, same set as Python.** Normal (unit/integration), benchmark,
  property, mutation, and fuzz — so a service's defensibility doesn't depend on
  what it's written in. (The Python equivalents live in `python-testing-standards`.)
- **Tests live next to the code.** Rust already co-locates unit tests under
  `#[cfg(test)]`; this is the pattern the Python side approximates.
- **Land it incrementally.** Prototype the pattern on one crate, prove it, then
  roll it out to the hot path of every service.

## The five kinds

| Kind | Question it answers | Tool | CI cadence |
| --- | --- | --- | --- |
| **Unit / integration** | Does it do the right thing? | `cargo nextest run` (+ `cargo test --doc`) | every PR (blocking) |
| **Benchmark + perf gate** | Is it still fast? | `criterion` / `iai-callgrind` + gate | every PR (blocking) |
| **Property-based** | Does it hold for *all* inputs? | `proptest` | every PR (blocking) |
| **Mutation** | Are the tests actually testing? | `cargo-mutants` | nightly / pre-merge (ratchet) |
| **Fuzz** | Does untrusted input crash it? | `cargo-fuzz` | nightly (time-boxed) |

Inherited tasks (`.moon/tasks/rust.yml`): `test` = `cargo nextest run`, `test-doc`
= `cargo test --doc`, `lint` = `cargo clippy --all-targets -- -D warnings`,
`audit` = `cargo-deny`. Library-level test tools (`criterion`, `proptest`) are
`[dev-dependencies]`; CLI tools (`cargo-nextest`, `cargo-mutants`, `cargo-fuzz`)
are proto vendored plugins.

Coverage (`cargo-llvm-cov`) is complementary, not one of the five — see
`references/coverage.md`. Integration tests that need a real Postgres or a mock S3
have their own pattern — see `references/integration.md`.

## File layout

Rust co-locates unit tests in the source file and puts cross-crate tests in
`tests/`. Benchmarks and fuzz targets are separate crates/targets:

```
my-crate/
├── src/lib.rs              # unit tests inline: #[cfg(test)] mod tests
├── tests/                  # integration tests (public API only)
├── benches/                # criterion / iai-callgrind benchmarks
└── fuzz/                   # cargo-fuzz crate (its own Cargo.toml)
```

## 1. Unit / integration — cargo-nextest

**The runner is `cargo-nextest`, not `cargo test`.** Nextest runs each test in its
own process with a proper scheduler: typically 2–3× faster on a multi-crate
workspace, with cleaner output, per-test timeouts, and automatic retry/flaky
detection. It's a near-free swap — one line in the moon task. Pin it as a **proto
vendored plugin** (`proto-plugins/cargo-nextest.toml`, mirroring `cargo-deny`),
not a dev-dependency; CI installs it from `.prototools` like every other tool.

Inline tests sit next to the code under `#[cfg(test)]` (compiled out of release):

```rust
pub fn clean(p: &str) -> String { /* ... */ }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inserts_space() { assert_eq!(clean("m11ae"), "M1 1AE"); }
}
```

`moon run <crate>:test` (= `cargo nextest run`) runs these. Integration tests in
`tests/*.rs` exercise the public API as an external user would, and run under
nextest too.

**The one caveat: nextest does not run doctests.** It cannot — doctests compile
differently. So if any crate has `///`-example doctests, they must be gated by a
separate task, or they silently stop running:

```yaml
tasks:
  test:                         # cargo-nextest — the fast main suite
    command: 'cargo nextest run'
    toolchain: 'system'
  test-doc:                     # doctests, which nextest can't run (no-op if none)
    command: 'cargo test --doc'
    toolchain: 'system'
```

Both run in CI under `moon ci`. If a crate has no doctests, `test-doc` is a cheap
no-op — keep it anyway so a future doctest can't slip through ungated.

## 2. Property-based — proptest

`proptest` (dev-dep) generates inputs and shrinks failures to a minimal case. The
`proptest!` macro lives inside `#[cfg(test)]`, so it runs under the normal `test`
task (`cargo nextest run`) — no extra task, still a PR gate:

```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    proptest! {
        #[test]
        fn clean_is_idempotent(s in ".*") {
            let once = super::clean(&s);
            prop_assert_eq!(super::clean(&once), once);
        }
    }
}
```

(`quickcheck` is the lighter alternative; prefer `proptest` for its shrinking.)

## 3. Benchmark + perf gate

Two complementary tools — pick per purpose:

- **criterion** (dev-dep) — statistical **wall-clock** benchmarks, great for local
  profiling and spotting big regressions. Run with `cargo bench`. It stores
  baselines under `target/criterion`; compare with `--baseline <name>` or the
  `critcmp` CLI.
- **iai-callgrind** (dev-dep, needs valgrind) — counts **CPU instructions** via
  cachegrind. This is the one to **gate on in CI**: instruction counts are
  deterministic and machine-independent, so a committed baseline doesn't drift
  across runners. Wall-clock criterion gates are flaky on shared CI; instruction
  counts are not.

```rust
// benches/matrix.rs (criterion)
use criterion::{criterion_group, criterion_main, Criterion, black_box};
fn bench(c: &mut Criterion) {
    let locs = make_locations(60);
    c.bench_function("travel_matrix", |b| b.iter(|| travel_matrix(black_box(&locs))));
}
criterion_group!(benches, bench); criterion_main!(benches);
```

```toml
# Cargo.toml
[[bench]]
name = "matrix"
harness = false        # required for criterion / iai-callgrind
```

moon task — gate on regression (criterion baseline + `critcmp`, or iai-callgrind's
built-in regression failure):

```yaml
tasks:
  bench:
    command: 'cargo'
    args: ['bench', '--bench', 'matrix', '--', '--baseline', 'committed']
    toolchain: 'system'
    options: { cache: false }   # runs in CI; iai-callgrind exits non-zero past threshold
```

`codspeed` (codspeed.io) is the managed alternative — it runs criterion benches in
an instruction-counted sandbox and reports regressions on the PR; worth it if you
want hosted history instead of a committed baseline.

## 4. Mutation — cargo-mutants

`cargo-mutants` rewrites the source (deletes a `?`, swaps an operator) and re-runs
the tests; survivors are untested behaviour. Point it at the same runner with
`--test-tool=nextest`. Slow → **not a PR gate**; nightly + ratchet. Add a proto
vendored plugin (`proto-plugins/cargo-mutants.toml`, mirror the `cargo-deny` one).

```yaml
  mutation:
    command: 'cargo-mutants'
    args: ['--test-tool=nextest', '--no-shuffle', '-j', '2']
    toolchain: 'system'
    options: { cache: false, runInCI: false }   # nightly workflow only
```

Each surviving mutant → add the unit/property test that would kill it.

## 5. Fuzz — cargo-fuzz

`cargo-fuzz` (libFuzzer, requires nightly Rust) lives in a `fuzz/` sub-crate. Run
time-boxed on a schedule, never per-PR. Proto vendored plugin for the CLI.

```rust
// fuzz/fuzz_targets/parse.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) { let _ = my_crate::clean(s); }
});
```

Run with a budget: `cargo +nightly fuzz run parse -- -max_total_time=300`. Commit
any crashing input under `fuzz/corpus/` and add it as a regression `#[test]`.

## How testing wires into a moon + proto monorepo

Three facts about the toolchain shape every decision here:

1. **moon is the task runner and the CI gate.** CI runs `moon ci`, which diffs
   against the base branch and runs the *affected* projects' tasks. Language-wide
   task defaults live in `.moon/tasks/rust.yml` (`inheritedBy: language`).
   Project-specific tasks (like a `bench` gate) go in that crate's `moon.yml`
   under `tasks:`. A task runs in CI unless it sets `options.runInCI: false`.

2. **proto pins every tool.** Versions live in `.prototools`. Built-in tools
   (rust) need no plugin; a standalone CLI binary (`cargo-nextest`, `cargo-mutants`,
   `cargo-fuzz`, `critcmp`, `cargo-llvm-cov`) needs a **vendored plugin** under
   `proto-plugins/*.toml` with a `file://` locator — a reviewed pointer to the
   tool's official releases, not a community plugin fetched at runtime;
   `cargo-deny` is the precedent. Dev-dependencies (`criterion`, `proptest`,
   `iai-callgrind`, `testcontainers`) are NOT proto tools — pin them in
   `[workspace.dependencies]` and reference `{ workspace = true }`.
   The `cargo-nextest` plugin has two gotchas worth copying verbatim: nextest's
   release tags are prefixed (`cargo-nextest-<version>`), so the plugin needs a
   `version-pattern` to extract the version; and macOS ships a single *universal*
   binary, so the macOS `download-file` hardcodes the arch.

3. **The hooks mirror CI.** `lefthook` runs `moon run :lint`/`:test --affected` on
   pre-push, so fast tests stay green locally before they ever reach CI. (`:test`
   is the nextest suite; doctests run via `test-doc` in CI.)

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
  `references/integration.md`.

## Rolling this out to a crate

1. Identify the **hot path(s)** and the **pure functions** — those drive the
   benchmark and property tests respectively.
2. Add the dev-dependency (library tools) or the proto vendored plugin (CLI tools).
3. Add/confirm the moon tasks: `test` (inherited), plus `bench` for the perf gate;
   mutation/fuzz as `runInCI: false` tasks invoked by a scheduled workflow.
4. Commit the benchmark **baseline** so the gate has something to compare against.
5. Make `moon ci` a required status check so the gates actually block merges.
