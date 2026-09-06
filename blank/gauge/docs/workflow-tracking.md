# Measuring a workflow, not a service

`gauge` is built for a **server**: something you keep alive, drive with load, and
size for peak concurrency. A **workflow** — a batch job, a CLI, an ETL script, a
training run — is the opposite shape. It starts, does its work, and exits. There
is no server to keep up and no traffic to synthesise, so you don't need a
container or a load driver. You need four numbers for the *whole run*:

- **wall time** — how long it actually took,
- **CPU cores used** — on average, and whether that saturates the machine,
- **peak memory** — will it fit / did it OOM,
- **which code** grew the heap — only if the peak is a problem.

This note is the simple sibling of the main tool: a couple of commands, no
containerisation. Reach for `gauge` itself only when the thing under test is a
long-running service (see the top-level README).

## CPU + time — cores, saturation, wall clock

Everything except heap attribution comes from one language-agnostic command:
`/usr/bin/time` (the binary, **not** the shell `time` builtin — quote or use the
full path).

```bash
# Linux (GNU time)
/usr/bin/time -v ./my_workflow --args

# macOS (BSD time — different flag, different output)
/usr/bin/time -l ./my_workflow --args
```

GNU `-v` prints a labelled block; the lines that matter:

```
Elapsed (wall clock) time (h:mm:ss or m:ss): 0:42.13
User time (seconds): 128.44
System time (seconds): 6.02
Percent of CPU this job got: 319%
Maximum resident set size (kbytes): 1048576
```

### Cores used, and is that the whole computer?

The scheduler doesn't report "cores" directly — you derive it. CPU time is summed
*across* cores, so dividing it by wall time gives the **average cores busy**:

```
cores_used = (user_seconds + system_seconds) / wall_seconds
```

For the run above: `(128.44 + 6.02) / 42.13 ≈ 3.2 cores`. GNU time hands you the
same figure pre-computed as **"Percent of CPU this job got"** — `319%` is `3.2`
cores. (BSD/macOS time doesn't print the percentage, so compute it from
`user`/`sys`/`real` yourself.)

To know whether that's the *full machine*, compare against the core count:

```bash
nproc                    # Linux: logical cores
sysctl -n hw.ncpu        # macOS: logical cores
```

`3.2` cores on an 8-core box means the job used **~40%** of the machine — ~4.8
cores sat idle. Two readings of that:

- **CPU-bound work leaving cores idle** → there's parallelism on the table
  (more workers / threads / processes would finish sooner).
- **The ratio is well under 1.0** → the job is **IO-bound** (disk, network,
  waiting), and more cores won't help; wall time won't drop by adding them.

If `cores_used ≈ nproc`, you're saturating the machine and wall time is now
CPU-limited.

### Wall time vs CPU time — don't confuse them

For a workflow, **wall time is the number that matters** ("how long is the
pipeline"). CPU time is the total work done across all cores and is almost always
larger on parallel jobs. A run can show 134 CPU-seconds in 42 wall-seconds
precisely *because* it used ~3 cores at once.

### One gotcha: peak-RSS units differ by OS

GNU time reports "Maximum resident set size" in **kbytes**; BSD/macOS time reports
it in **bytes**. The same 1 GB run prints `1048576` on Linux and `1073741824` on
macOS. Divide accordingly before comparing.

## Memory — where did the heap go?

`/usr/bin/time` gives peak RSS (the black-box "how much"). When the peak is a
problem, you want the **which code** — a heap profiler run over the same command.

### Python — memray on any command

`memray` traces every allocation, including native ones (NumPy/pandas/C
extensions) that `tracemalloc` misses. Run it over an arbitrary script or module,
then render:

```bash
# profile a script (or `-m package.module` for a module entry point)
uv run --with memray python -m memray run -o profile.bin ./my_workflow.py --args

# then, from the same capture:
python -m memray stats     profile.bin   # peak heap + total allocated, in the terminal
python -m memray summary   profile.bin   # top allocating functions
python -m memray flamegraph profile.bin  # -> profile.html, the full call tree
```

Notes:
- memray measures **allocated heap** (with an exact **peak**), which for a
  one-shot job is exactly the "why did this OOM" question — it points at the line
  holding the peak.
- add `--native` to `memray run` for C/native stack frames (needs debug symbols;
  slower).
- the flamegraph HTML references a CDN, so it's not fully offline.

For a leak that's purely Python-level, `tracemalloc` (stdlib, zero install) is the
lighter first reach — see `python-approaches.md` for the wider landscape.

### Rust — dhat or heaptrack on any binary

Two routes, depending on whether you can recompile:

- **You own the source (portable, recommended): `dhat-rs`.** Same lens the
  `examples/axum/` heap probe uses. Add `dhat` as a dev-dependency behind a
  feature, and drop a profiler guard at the top of `main`:

  ```rust
  #[cfg(feature = "dhat-heap")]
  #[global_allocator]
  static ALLOC: dhat::Alloc = dhat::Alloc;

  fn main() {
      #[cfg(feature = "dhat-heap")]
      let _profiler = dhat::Profiler::new_heap();
      // ... workflow ...
  }
  ```

  Run `cargo run --release --features dhat-heap`; it writes `dhat-heap.json`, which
  you open in the DHAT viewer. Works on macOS and Linux, no external tooling.

- **You can't touch the source (no recompile, Linux): `heaptrack`.** Wrap the
  built binary:

  ```bash
  heaptrack ./target/release/my_workflow --args
  heaptrack_gui heaptrack.my_workflow.*.zst   # or heaptrack_print for a terminal report
  ```

  `valgrind --tool=dhat ./target/release/my_workflow` is the cross-platform
  fallback (feeds the same DHAT viewer) but runs the program under Valgrind, so
  it's much slower and skews timing — use it for the allocation picture, never for
  the CPU/time numbers above.

Build `--release` before profiling memory: a debug binary's allocation profile
isn't the one you ship. And remember the Rust gotcha from the examples —
`vec![0u8; N]` reserves lazily-zeroed pages that aren't resident until written, so
peak RSS can trail peak *allocated* heap.

## Putting it together

The CPU/time command and the heap profiler compose in one invocation — outer
`/usr/bin/time` for the black-box totals, inner profiler for attribution:

```bash
/usr/bin/time -v uv run --with memray python -m memray run -o profile.bin ./my_workflow.py
```

That single run tells you wall time, cores used, peak RSS *and* leaves a
`profile.bin` you can flamegraph if the peak looks wrong.

## When to reach back for gauge

Use the full `gauge` tool instead of this note when the target is a **service you
have to keep alive while you generate load** — a web API, a queue consumer, a
model server. There the "peak" only appears *under* traffic, so gauge
containerises the app, drives it, and reads the kernel's cgroup counters. A
workflow that runs to completion on its own needs none of that — `time` plus a
heap profiler is the whole toolkit.
