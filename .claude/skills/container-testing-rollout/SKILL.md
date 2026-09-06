---
name: container-testing-rollout
description: >-
  Roll out container-image testing onto a project — build the *production image*
  and test the artifact you actually ship, across three axes: runtime (does it
  behave when containerised, via testcontainers), structure (is it assembled
  correctly, via container-structure-test), and hygiene (is it safe and lean, via
  Trivy) — plus a fourth, whole-system axis: capacity (does it fit its resource
  budget, and how does it degrade when starved), by sandboxing the running app
  under real cgroup CPU/memory limits and stress-testing it with k6. Wires the
  three image axes as moon tasks that each depend on an image-build step, so you
  can never test a stale image, with the two image-native tools proto-pinned as
  reproducible host binaries. Use whenever asked to "add container testing", "test
  my docker/podman image", "scan the image for vulnerabilities / CVEs", "add
  trivy", "add container-structure-test", "testcontainers for my image", "test the
  image I deploy", "roll out image testing", "size the app's CPU/memory", "stress
  test the container", "resource curve / knee", "sandbox the app under limits",
  "run the whole app in a limited container", "spin up a disposable seeded app to
  explore / demo", "give an agent a real app to drive", "explorable sandbox",
  "live seeded environment", or to reproduce the nix_stack
  three-axes container-testing setup. Covers Rust (scratch) and Python (distroless
  / slim) images and honours the podman/docker socket wiring.
---

# Roll out container-image testing (the three axes)

This skill reproduces a battle-tested way of testing a **production container
image** on a target project. The core reframing, in one breath:

> We deploy a **container**, so we test the container — build the prod image and
> exercise _it_ locally, before it deploys. The image you test is the image that
> runs in prod: same base, same entrypoint, same user, same files.

An image can be wrong in three independent ways, so there are **three image
axes** — each answering a different question with the tool native to it. Forcing
all three through one `bash` + `curl` smoke script covers only the first, and
imperatively. A fourth axis asks a question none of the three can: not _is the
image correct_ but _is it correctly **sized**_ — which only shows up when the
whole system runs under load, against its dependencies, under real limits.

| Axis          | Question                                             | Tool                     | Scope        | Language-specific? |
| ------------- | ---------------------------------------------------- | ------------------------ | ------------ | ------------------ |
| **Runtime**   | Does it behave when containerised?                   | testcontainers           | one image    | **yes**            |
| **Structure** | Is the image assembled correctly?                    | container-structure-test | one image    | no                 |
| **Hygiene**   | Is it safe and lean?                                 | Trivy                    | one image    | no                 |
| **Capacity**  | Does it fit its budget, and degrade well when starved? | sandbox + k6           | whole system | no\*               |

The first three are **per-image and hermetic** — they inspect the built artifact
at rest. The load-bearing property among them: **only the runtime axis depends on
the language.** Structure and hygiene operate on the _built image_ — its files,
its metadata, its vulnerabilities — which look the same whatever the binary inside
was written in. That is what makes the image rollout portable: porting from Rust
to Python swaps **one of the three tools** and leaves the other two, and the whole
moon task graph, untouched.

The fourth axis is different in kind. **Capacity** runs the _whole system_ — the
production image, its built frontend, and a disposable seeded warehouse — together
on a private network under **cgroup CPU/memory limits**, then throws k6 load at it
to find the **resource curve**. Its orchestration is shared and language-agnostic
(\*it assumes a FastAPI + built-SPA app that resolves its DB from an env var); the
app supplies only a seed function and a load script. It is optional and heavier
than the other three — reach for it when you need to _size_ a deployment, not on
every build. See **The fourth axis — capacity** below.

Templates referenced below live in `templates/` next to this file. The hard part
— the two vendored proto plugins — are ready to copy **verbatim**.

## Before you start: read the reference

`references/architecture.md` — the _why_ behind each image axis and the
non-obvious gotchas. **Read it first.** It will save you from the traps: the
`podman save` tarball pivot (why structure + hygiene never mount a daemon socket),
the testcontainers socket wiring for Podman, `toolchain: 'system'` + `cache:
false` on every image task, the fully-qualified `IMAGE_REF` Podman needs, and the
engine-availability skip guard that keeps the suite green on a machine with no
container engine.

`references/capacity-sizing.md` — the _why_ and the gotchas for the **fourth
axis**: the sandbox that runs the whole app under real limits, how to read the
resource curve, and the traps (cgroups only bite inside a container, schema-name
casing, deterministic self-healing names, the connection pool that is usually the
real bottleneck). **Read it before rolling out capacity** — skip it if you are
only doing the three image axes.

## Prerequisites — what the target must already have

This skill tests an image; it does not invent one. Confirm the target has:

1. **A Dockerfile that builds a runnable image.** Ideally a lean one — `scratch`
   for a static Rust/Go binary, `gcr.io/distroless/*` or `*-slim` for an
   interpreted app. See `templates/Dockerfile.rust-scratch` and
   `templates/Dockerfile.python-distroless` for the shape the assertions assume.
2. **proto + moon already in place** (a `.prototools` at the root and a `moon.yml`
   per project). If not, roll those out first with the **moon-proto-rollout**
   skill — this skill _adds tasks and tools to_ that stack, it does not set it up.
3. **A reachable container engine** — Podman or Docker. The runtime test skips
   cleanly without one, but you need it to actually run the axes locally.

## Procedure

Work top-down. Confirm scope with the user where marked ⚠.

### 1. Survey the target

Determine, by inspection (don't assume):

1. **The image's contract** — read the Dockerfile. What is the entrypoint, the
   exposed port, the user, the key env vars, and which files get copied in? These
   become the _structure_ assertions. What HTTP endpoints does it serve, and what
   log line does it print once it is bound and serving? These become the
   _runtime_ test's `WaitFor` and assertions.
2. **The language of the app inside** — this picks the runtime test framework
   (Rust → `cargo-nextest` + the `testcontainers` crate; Python → `pytest` +
   the `testcontainers` PyPI package). ⚠ Nothing else keys off the language.
3. **The image name/tag** the build produces (e.g. `myapp:prod`), and whether the
   team uses **podman or docker** — Podman needs fully-qualified refs
   (`localhost/myapp:prod`) and a little socket wiring the wrappers handle.

### 2. Pin the two image-native tools with proto

Copy both manifests from `templates/proto-plugins/` into the target's
`proto-plugins/`, verbatim — they are reviewed pointers at the tools' _official_
releases and need no edits:

- `container-structure-test.toml`
- `trivy.toml`

Then add them to the root `.prototools` — a version line under the top-level
shorthand block **and** a `file://` locator under `[plugins]`:

```toml
# — versions —
trivy = "latest"
container-structure-test = "latest"

# — vendored plugins —
[plugins]
trivy = "file://./proto-plugins/trivy.toml"
container-structure-test = "file://./proto-plugins/container-structure-test.toml"
```

Why host binaries rather than `aquasec/trivy:latest` images? Reproducibility: a
proto-pinned binary has an exact version and no `:latest`-image drift or
cross-arch emulation. Verify with `proto install` — both resolve and install.

> testcontainers is **not** a proto tool. It is a dev-dependency of the app's own
> test suite (a Cargo dev-dep or a `uv` dev group), because it is the one
> language-bound axis. You add it in step 4, not here.

### 3. Add the moon tasks (the task graph is the spine)

Copy the task block from `templates/moon-image-tasks.yml` into the project's
`moon.yml`. It defines five tasks; the shape is identical for every language and
only the body of `image-test` changes:

- `image-build` — builds the prod image. **Not cached** (`cache: false`): the
  built image lives in the engine's store, not the workspace, so moon can't hash
  it as a file output.
- `image-test` — **runtime**. Drives the live container over HTTP via
  testcontainers. The only language-specific task.
- `image-structure` — **structure**. `container-structure-test` against a saved
  tarball.
- `image-scan` — **hygiene**. Trivy against a saved tarball, failing on
  HIGH/CRITICAL.
- `image-check` — depends on all three; one command for the lot.

Two invariants make this work, both spelled out in the template comments:

- **Every test task `deps: ['~:image-build']`** — so you can never test a stale
  image; moon rebuilds it first.
- **Every image task is `toolchain: 'system'` and `cache: false`** — these shell
  out to podman/trivy/etc. (not a moon-managed toolchain), and their effect is a
  live container or a fresh scan, never a cacheable file output.

The **`podman save --format docker-archive` tarball is the pivot** for the two
image-native axes: they read the saved tarball with `--driver tar` / `--input`,
so no daemon socket is ever mounted. Only `image-test` needs the live socket,
because only it manages a running container.

Fill in the image name/tag and, for structure, the config path.

### 4. Write the runtime test (the one language-specific axis)

Pick the template for the app's language and drop it beside the app's other
tests:

- **Rust** → `templates/image_it.rs` → `<crate>/tests/image_it.rs`. Add the
  `testcontainers` + an HTTP client (`reqwest`) as dev-deps. Run via
  `cargo-nextest`.
- **Python** → `templates/test_image.py` → `tests/test_image.py`. Add
  `testcontainers` + `httpx` to the dev group. Run via `pytest`.

Both templates carry the two things that make the runtime axis robust, and you
should keep them:

- **An engine-availability skip guard** — if no podman/docker responds to `info`,
  the test prints a skip and returns green. This mirrors how the Postgres/S3
  integration tests behave, so the suite never fails a machine without an engine.
- **A `WaitFor` on the real "serving" log line** — testcontainers blocks until the
  container prints it, so the test never races the server's startup. Replace the
  placeholder log line with the one the target actually prints.

Edit the asserted endpoints, the exposed port, and the wait line to match the
target's contract (from step 1).

### 5. Write the structure assertions

Copy `templates/structure-test.yaml` to the project root and rewrite the
asserted values to lock the Dockerfile's contract from step 1:

- `fileExistenceTests` — the files you actually COPY into the image (the binary,
  the static assets, `/app`, the interpreter …).
- `metadataTest` — `user`, `exposedPorts`, `entrypoint`, and the env vars you
  set. This is what catches a Dockerfile refactor that silently drops the
  non-root user or mislays the assets — things a runtime test might not notice.

### 6. Validate the rollout

Run each axis, then all three:

```bash
proto install                       # trivy + container-structure-test resolve
moon run <project>:image-build      # the prod image builds
moon run <project>:image-test       # runtime   (testcontainers)
moon run <project>:image-structure  # structure (container-structure-test)
moon run <project>:image-scan       # hygiene   (Trivy)
moon run <project>:image-check      # all three, one command
```

Confirm `image-test` actually starts and reaches the container (not just the skip
path — an accidental permanent skip is a silent hole). On a lean base, `image-scan`
should be near-empty — which is itself the argument for a minimal base: nothing to
scan means almost nothing to exploit. Hygiene is where the base image pays off as
a _number_, not a claim.

## Adapting, not transplanting

The target will differ. Do **not** blind-copy the `nix-stack` name, the `/dist`
paths, or the `:3000` port — derive every asserted value from the target's own
Dockerfile and endpoints. The _structure_ transfers; the _contents_ are the
target's.

The one real porting difference is the **Dockerfile, not the tests**: a Python
app can't target `scratch` (it ships an interpreter and `site-packages`), so its
minimal base is one tier up — `distroless/python3` or `python:*-slim`. That means
the structural assertions describe a different filesystem, and Trivy now has an OS
and packages to actually scan, so the hygiene axis does more work and matters
more. The test _strategy_, the task names, the dependency graph, and the tarball
pivot are all unchanged.

## The fourth axis — capacity (sizing under load)

The three image axes prove the artifact is _correct_. They cannot tell you _how
much CPU and memory it needs_, or how it behaves when it doesn't get enough —
because that only surfaces when the whole system runs, against its dependencies,
under real limits, with sustained load. Guessing over-provisions (wasted spend) or
under-provisions (latency spikes, OOM kills); both surface only in production. The
capacity axis answers those two questions _before_ deploying.

Read `references/capacity-sizing.md` first — it carries the why and the gotchas.
The mechanism, in one breath:

> Run the **production image** in its own container, on a private network with a
> **disposable seeded warehouse**, under real cgroup `--cpus`/`--memory` limits,
> then throw k6 load at it and watch latency and errors move as you dial the
> budget. The knee of that curve is the efficient allocation.

Two facts make it honest and cheap:

- **cgroup limits only bite inside a Linux container.** A host process on macOS
  can't be CPU-throttled, so running the app _as its prod container_ (not host
  uvicorn) is the whole point — it is what makes `--cpus` mean something.
- **The app already resolves its DB from an env var** (`DB_CONNINFO_OVERRIDE`), so
  the sandbox points it at the warehouse container with one variable — no
  app-side change to make it sandbox-aware.

### When to roll it out

The three image axes belong on every project — they gate each build. Capacity is
**optional and heavier**: reach for it when you need to _size a deployment_ (a new
app going to the platform, a resource complaint, a suspected leak), not on every
commit. It also assumes a **FastAPI + built-SPA** app whose orchestration our
shared runner already knows. If the target isn't that shape, stop and say so
rather than forcing it.

### Rolling it out (given the shared runner exists)

The orchestration lives in a shared module (`common_py.testing.sandbox`,
`run_containerised_sandbox`) that owns the network, the limits, and the cleanup —
you do **not** re-implement any of that. A new app supplies only its specifics
(the `seed` coroutine and the CLI command are both in `templates/seed_world.py`):

1. **Write a `seed(db, env)` coroutine.** Create the source tables the target
   endpoints read and insert the minimum realistic rows; leave join-target tables
   created-but-empty so their LEFT JOINs resolve to zero rows instead of erroring.
   Grant the dev identity the permissions the app's menus need. Reuse the same
   generators the end-to-end tests seed with, so sandbox and suite agree.
2. **Add a `sandbox-container` CLI command** (a few lines) that passes the app
   name, the `seed` coroutine, and `--cpus`/`--memory`/`--port` through to
   `run_containerised_sandbox`.
3. **Write a k6 load script** — copy `templates/stress.k6.js` and point it at the
   endpoints a real page hits, keeping to the scopes your seed populates, with
   thresholds (`p95`, `p99`, error rate) so a too-tight config **fails the run**
   instead of hiding in the averages.

### Reading the resource curve

Boot at a fixed budget, drive a **fixed** k6 load, and sweep **one** resource axis
with everything else held — the load is the control variable:

```bash
# terminal 1: the sandbox at one budget
uv run <app> sandbox-container --cpus 1 --memory 512m --port 8070

# terminal 2: the same load against it
BASE_URL=http://127.0.0.1:8070 VUS=50 DURATION=1m k6 run modules/<app>/stress/<app>.js
```

Sweep `--cpus` (memory fixed), then `--memory` (cpus fixed), exporting each run's
summary. Plot p95 and error rate against the axis; three regions appear:

- **Starved** — steep latency, rising errors: resource-bound.
- **The knee** — where it flattens; past here more resources buy little. **The
  config just past the knee that meets your SLO is the efficient allocation.**
- **Over-provisioned** — flat; you pay for idle capacity.

Then, at that chosen budget, sweep the **load** (VUS) to find the capacity ceiling
— the throughput at which p95 climbs again. That (allocation, ceiling) pair is
what a deployment-sizing decision needs.

**The bottleneck is often not CPU.** For a DB-backed app the first limit is
usually the **connection pool** — past its max in-flight queries, requests queue
while CPU sits idle and p99 climbs. When a CPU curve stays flat while latency is
bad, suspect a fixed-size resource (the pool) before adding cores; make the pool
size env-driven and it becomes a stress axis of its own.

The worked example is `container_dock.md` at the repo root (orders); the shared
runner is `common_py.testing.sandbox`.

## The explorable sandbox — a disposable world to poke at

The capacity axis runs the app to _measure_ it. The **same module** runs the app
to _explore_ it. `run_sandbox` boots the identical seeded world — the real
production app against a throwaway seeded Postgres, zero path to prod — but as
plain host uvicorn, then simply blocks until Ctrl-C. No cgroup limits, no k6: the
point here is not a number, it is a **living app** that a human — or an agent
driving a browser — can click through with realistic data.

It is the **sibling** of the capacity sandbox, not a fifth axis: it asserts
nothing and gates no build. Everything load-bearing is shared — the same
`seed(db, env)` coroutine, the same three-layer safety guard (`enable_test_safety`
strips the Redshift creds and pins the sentinel _before_ any pool opens), the same
deterministic self-healing container name. The one deliberate difference from
capacity is _where the app runs_: on the host (uvicorn in a thread), because you
do not need cgroup limits to explore — and host uvicorn buys you hot logs and a
real port with **no image build**.

### When to reach for it

- Hand an **agent** a real app to drive (pairs with **playwright-webapp-testing**
  and **forage**): a genuine backend plus seeded data, safe to hammer.
- Manual QA or a demo of a feature against realistic data, without touching a
  shared environment.
- Reproduce a data-shaped bug: seed the exact rows, boot, click.

### Rolling it out (given the shared runner exists)

The same one-line move as capacity — you supply only the specifics:

1. **Reuse the `seed(db, env)` coroutine** you (or the e2e suite) already wrote,
   so the sandbox and the tests seed from the same generators and agree.
2. **Add a `sandbox` CLI command** that passes the app import string, the frontend
   dir and the `seed` coroutine to `run_sandbox` (a `--port` / `--no-build`
   passthrough is enough).

Both pieces — the world-building `seed` and the two CLI commands — are one file:
copy `templates/seed_world.py`, fill in its `EDIT` markers, and the same `seed`
feeds this explorable sandbox and the capacity one.

```bash
uv run <app> sandbox --port 8060
# → SANDBOX READY — disposable seeded world at http://127.0.0.1:8060
```

Reach for the **containerised** sandbox (the capacity axis) only when you need
real resource limits; for everything else — exploration, demos, agent-driving —
host `run_sandbox` is lighter and starts faster, because it skips the image build.
