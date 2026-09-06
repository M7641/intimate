# The capacity axis — sizing under load (the why and the gotchas)

Read this before rolling out the fourth axis. SKILL.md tells you *what* to do;
this explains *why* the sandbox is shaped the way it is, and the traps that will
otherwise bite you. If you are only doing the three image axes, skip this file.

## The shift: from "is the image correct" to "how big should it be"

The three image axes inspect the built artifact at rest. None of them can answer
the two questions that decide a deployment's allocation:

- **How much CPU and memory does it actually need?**
- **How does it behave when it doesn't have enough?**

Both only surface when the *whole system* runs — the real app image, its
dependencies, real limits, sustained load. Guessing over-provisions (wasted
spend) or under-provisions (latency spikes, OOM kills), and either way the truth
only appears in production. The capacity axis reproduces just enough of
production, locally and disposably, to answer them first.

## The core idea: run the real artifact, dial its budget

A normal dev run talks to the real warehouse and runs uvicorn on the host —
neither disposable nor resource-limited. The sandbox replaces both halves:

- The **warehouse** becomes a throwaway Postgres container, seeded with a
  realistic-enough world and torn down on exit. No credentials, no path to prod.
- The **app** runs as the **production image** in its own container, on a private
  network with the warehouse, under **real cgroup CPU/memory limits**.

Because the app container is the exact artifact we deploy, talking to a real (if
small) SQL database over a real network under real limits, the environment matches
production in the ways that matter for sizing. Only the *scale* of the data
differs — fine, because we measure behaviour per unit of load, not warehouse
capacity.

## The two facts that make it honest and cheap

### 1. cgroup limits only bite inside a Linux container

A host process on macOS cannot be CPU-throttled — `--cpus` on host uvicorn is a
no-op. Running the app **as its production container** (not host uvicorn) is the
whole reason for the containerised mode: it is what makes `--cpus` and `--memory`
actually enforced by the kernel inside the Podman/Docker VM.

### 2. The app resolves its DB from an env var

The app already reads its database from `DB_CONNINFO_OVERRIDE`. So the sandbox
points the containerised app at the Postgres container over the private network
with a single variable rewritten to the in-network host (`sandbox-db:5432`) — no
code change to the app to make it sandbox-aware.

## The gotchas (each already handled by the shared runner)

- **Schema-name casing.** In-process tests monkeypatch the env manager to align
  the app's uppercase prod schema with the lowercase one the seeders create. A
  separate container can't be monkeypatched. **Reads** work anyway because the app
  renders the schema as a bare word and Postgres folds unquoted identifiers to
  lowercase. **Writes** quote the identifier (case-sensitive) and would need an
  uppercase seed — so a read-focused stress run needs no extra work, a
  write-focused one does.

- **Deterministic names + self-healing.** Containers and the network get fixed
  names. A clean exit tears them down; a `kill -9` before teardown leaves them, so
  the next boot force-removes any stale namesake first. Boot is idempotent
  regardless of how the last run ended.

- **Safety before any pool opens.** The runner strips `REDSHIFT_*` creds and pins
  a sentinel conninfo before anything can open a connection, and wires
  `DOCKER_HOST` to the Podman socket. Nothing in the sandbox can reach prod.

- **The image build is minutes; a run is seconds.** The prod image is built once
  and cached; later runs reuse it (force with `--rebuild-image`). Don't rebuild
  per sweep step.

## Reading the curve: the load is the control variable

Sweep **one** resource axis while holding load and the other axis fixed. Keep
VUS/DURATION identical across a sweep — vary only `--cpus` (or only `--memory`).
Plot p95 and error rate against the axis; three regions appear:

- **Starved** — steep latency, rising errors: the app is resource-bound.
- **The knee** — where the curve flattens; past here more resources buy little.
  The config just past the knee that still meets your SLO is the efficient
  allocation.
- **Over-provisioned** — flat; extra cores/RAM change nothing, you pay for idle.

Then hold that budget and sweep the **load** (VUS) to find the capacity ceiling —
the throughput at which p95 climbs again. The (allocation, ceiling) pair is the
deployment-sizing answer.

## The bottleneck is often not CPU

For a DB-backed app the first limit is usually the **connection pool**, not the
CPU. Past the pool's max in-flight queries, requests queue on the pool while CPU
sits idle and p99 climbs. When a CPU curve stays flat while latency is bad,
suspect a fixed-size resource (the pool) before adding cores. Make the pool size
env-driven and it becomes a stress axis of its own — frequently the
highest-leverage one.

## A second axis: constraining the warehouse

The Postgres container can take the same `--cpus`/`--memory` limits to simulate a
constrained warehouse — a useful extra axis when the DB is the suspected
bottleneck rather than the app.
