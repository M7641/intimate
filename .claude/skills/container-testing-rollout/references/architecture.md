# Container-image testing — the why and the gotchas

Read this before rolling out. The procedure in SKILL.md tells you *what* to do;
this explains *why*, and the non-obvious traps that will otherwise bite you.

## The shift: test the artifact you ship

We deploy a container. So rather than aligning the dev environment with prod
byte-for-byte, we do the reverse: **build the prod image and test _it_, locally,
before it deploys.** The container you exercise is the container that runs in
production — not a stand-in with different libraries, a different entrypoint, or a
different user. That reframes "test the app" into "test the image".

An image can be wrong in three independent ways, so there are three axes, each
with the tool native to it. A single `bash` + `curl` smoke script covers only the
first axis, and imperatively.

## Why the split matters: only runtime is language-specific

- **Runtime** (testcontainers) starts the real container and drives it over HTTP.
  It depends on the app's test runner, so it is the one language-bound axis.
- **Structure** (container-structure-test) and **hygiene** (Trivy) operate on the
  *built image* — its files, its metadata, its CVEs — which look identical whether
  the binary inside is Rust, Python, or Go.

That is the whole portability argument: porting the rollout to another language
changes **one of the three tools** and leaves the task graph, the tarball pivot,
and the other two axes untouched.

## The two load-bearing implementation choices

### 1. The `podman save` tarball is the pivot

Structure and hygiene run their tools against a **saved docker-archive tarball**
(`podman save --format docker-archive`), read with `--driver tar` (structure) and
`--input` (hygiene). No daemon socket is mounted for either. podman's role in
these two axes is reduced to *building and exporting* the image.

Only the runtime axis needs the live socket, because only it manages a running
container lifecycle. Keeping the socket out of two of the three axes is why they
are simple, hermetic, and identical across languages.

### 2. The tools are proto-pinned host binaries, not images

`container-structure-test` and Trivy each have a vendored `proto-plugins/*.toml`
manifest pointing at the tool's *official* upstream releases. So their versions
are reproducible — no `:latest` image tag, no cross-arch emulation, no pulling a
community plugin at runtime. Only testcontainers is language-bound (a dev-dep of
the app, not a proto tool).

## The gotchas (each already handled in the templates)

- **`image-build` must not be cached.** The built image lives in the container
  engine's store, not the workspace, so moon can't hash it as a file output.
  `options: { cache: false }`. The same holds for every image task: their effect
  is a live container or a fresh scan, never a cacheable output.

- **Every image task is `toolchain: 'system'`.** They shell out to
  podman/trivy/container-structure-test — host binaries on PATH (via proto
  shims), not a moon-managed toolchain. Without `system`, moon tries to run them
  through a language toolchain and fails.

- **Every test task `deps: ['~:image-build']`.** This is what guarantees you can
  never test a stale image — moon rebuilds it first. It is the single most
  important edge in the graph.

- **Podman needs fully-qualified image refs.** `myapp:prod` resolves under Docker
  but Podman wants `localhost/myapp:prod`. The runtime wrapper passes
  `IMAGE_REF=localhost/myapp:prod`; the test splits it back into name + tag.

- **testcontainers + Podman needs socket wiring.** The runtime task detects the
  Podman machine socket into `DOCKER_HOST` and sets
  `TESTCONTAINERS_RYUK_DISABLED=true` — Podman auto-removes containers, and ryuk
  (testcontainers' reaper) cohabits poorly with that. On Docker this is a no-op.

- **The runtime test must skip, not fail, without an engine.** Both templates
  probe `podman|docker info` and return green if neither responds. This mirrors
  the Postgres/S3 integration tests, so CI or a laptop without an engine stays
  green. The flip side: when validating, confirm the test actually *reaches* the
  container — an accidental permanent skip is a silent hole.

- **`WaitFor` the real serving line.** testcontainers blocks until the container
  logs the line you name, so the test never races startup. If you pick a line the
  app doesn't print, the test hangs until timeout. Copy the exact string the app
  logs once it is bound (Rust) or "startup complete" (Python/uvicorn).

## Hygiene is where a minimal base pays off as a number

On a `scratch` image, Trivy's result is near-empty — nothing to scan means almost
nothing to exploit. That is the argument for `scratch` expressed as a measurement,
not a claim. A Python app can't reach `scratch`, so its base is `distroless` or
`slim`: leaner-is-fewer-CVEs becomes a live, gated number on every build.
