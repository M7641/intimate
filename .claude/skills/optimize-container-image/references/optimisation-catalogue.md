# Optimisation catalogue

Each technique below is one iteration of the loop in `SKILL.md`: apply it alone,
rebuild, run the oracle, measure, keep or revert. They're ordered by
impact-per-risk — start at the top. For each: **what**, **why it helps**, the
**failure mode** it tends to trip, and **how the oracle catches it**.

---

## 1. Multi-stage build

**What.** Split the Dockerfile into a *builder* stage (has compilers, dev
headers, package managers) and a *final* stage that `COPY --from=builder` only
the finished artifact.

```dockerfile
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/app /usr/local/bin/app
ENTRYPOINT ["app"]
```

**Why.** The toolchain (often hundreds of MB) never reaches the shipped image.
Usually the single biggest reduction.

**Failure mode.** The artifact has a runtime dependency (shared lib, cert
bundle, template/static dir) that lived in the builder but wasn't copied.

**Oracle catches it.** Part 2 (it runs) fails immediately — the binary can't
find `libssl`, or the app 500s on a missing template. Read `podman logs`; the
missing path is usually named.

---

## 2. Right-size the final base image

**What.** Move the final stage to the smallest base that still runs the app:
`-slim` → `alpine` → `distroless` → `scratch`, in decreasing size and increasing
strictness.

- **scratch** — empty. Only for fully static binaries (Go, Rust with musl/static
  linking). No shell, no libc, no certs.
- **distroless** — libc + certs, no shell/package manager. Great for compiled
  langs and slim Python.
- **alpine** — tiny, but musl libc (not glibc) can break glibc-compiled binaries
  and some Python wheels.
- **-slim** (Debian) — safest shrink; keeps glibc and a shell.

**Why.** The base is often the largest single contributor after the toolchain.

**Failure mode.** Missing libc variant (alpine's musl vs glibc), missing CA
certs (TLS calls fail), missing shell (an `ENTRYPOINT`/`CMD` in shell form, or a
healthcheck using `sh`, breaks), missing timezone data.

**Oracle catches it.** Part 2 or 3: the app won't start, or outbound HTTPS fails
with a cert error, or a shell-form entrypoint errors "sh: not found". For
`scratch`, add certs explicitly:
`COPY --from=builder /etc/ssl/certs /etc/ssl/certs`.

---

## 3. `.dockerignore`

**What.** Add a `.dockerignore` excluding everything not needed by the build:
`.git`, `node_modules`, `target/`, `__pycache__`, local env files, test
fixtures, the scratchpad.

**Why.** Shrinks the build *context* podman uploads — faster builds, and it
stops secrets/junk from sneaking into layers via a broad `COPY . .`.

**Failure mode.** Over-exclusion: you ignore a file the build actually needs.

**Oracle catches it.** Part 1 (build) fails — `COPY` can't find the file, or a
compile step misses a source.

---

## 4. Layer & cache ordering

**What.** Copy dependency manifests and install dependencies *before* copying
application source.

```dockerfile
# deps layer — cached until the manifest changes
COPY pyproject.toml uv.lock ./
RUN uv sync --frozen --no-dev
# source layer — changes often, but doesn't bust the deps cache above
COPY . .
```

**Why.** Doesn't shrink the final image, but a code edit no longer re-installs
all dependencies — build time drops from minutes to seconds on the common path.

**Failure mode.** Low risk. Occasionally an install step needs a source file
that now arrives later; the build fails until you reorder.

**Oracle catches it.** Part 1 (build) fails fast. Verify the *win* by timing two
builds with a trivial source edit between them.

---

## 5. Fewer, leaner `RUN` layers

**What.** Combine related `RUN` steps and clean caches *within the same layer*.

```dockerfile
RUN apt-get update \
 && apt-get install -y --no-install-recommends curl ca-certificates \
 && rm -rf /var/lib/apt/lists/*
```

**Why.** Each `RUN` is a layer; deleting a file in a *later* layer doesn't
reclaim its space (the earlier layer still carries it). Cleaning in the same
`RUN` is what actually shrinks the image. `--no-install-recommends` avoids
pulling optional extras.

**Failure mode.** Over-aggressive cleanup removes something the app needs at
runtime (e.g. deleting certs, or purging a lib that's not build-only).

**Oracle catches it.** Part 2/3: runtime error for the removed file.

---

## 6. Drop build-only dependencies from the final image

**What.** Ensure compilers, `-dev` headers, and build tools live only in the
builder stage (a consequence of doing #1 well). If not multi-stage, install
build deps and remove them in the same `RUN` after building.

**Why.** Build toolchains are pure runtime dead weight.

**Failure mode.** A "build-only" dep was actually a runtime dep (e.g. a dynamic
library the binary links against, not just its `-dev` headers).

**Oracle catches it.** Part 2: dynamic-link error at startup. Fix by installing
the *runtime* package (not the `-dev` one) in the final stage.

---

## 7. Pin, and run as non-root (hardening — often no size change)

**What.**
- **Pin** the base image to a specific tag/digest (`debian:bookworm-slim`, or a
  `@sha256:` digest) instead of `latest` — reproducible rebuilds.
- **Pin** language/tool versions in the manifest.
- Add a non-root user and `USER app` in the final stage.

**Why.** Reproducibility and least-privilege. Not a size win, but this loop is
also where "harden" belongs, and it's cheap to verify.

**Failure mode.** Non-root user can't write to a path the app assumes is
writable, or can't bind a port < 1024.

**Oracle catches it.** Part 2: permission-denied on write or bind. Fix by
`chown`ing the needed dir to the user, or using a port ≥ 1024.

---

## Quick reference — commands used in the loop

```bash
podman build -t app:try .                          # rebuild after one change
podman images app:try --format '{{.Size}}'         # size after
podman history app:try --human                     # per-layer breakdown (find targets)
podman run --rm app:try --help                     # cheap "it runs" for a CLI
podman run -d -p 8080:8080 app:try                 # start a service to probe
podman logs <cid>                                  # first stop when the oracle fails
podman image prune -f                              # reclaim disk between iterations
```
