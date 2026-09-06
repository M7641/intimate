---
name: optimize-container-image
description: >-
  Shrink and harden a container image the safe way — as a guarded loop, not a
  rewrite. Build the image in podman, prove it works (build + run + a test
  command that is your "still works" oracle), then apply optimisations ONE AT A
  TIME, re-running the oracle after each so a size win never silently breaks the
  app. Carries the ordered optimisation catalogue (multi-stage builds, slimmer
  base, .dockerignore, layer/cache ordering, fewer & leaner RUN layers, dropping
  build-only deps, non-root, pinning) and the measure → change → verify → keep
  or revert discipline that ties them together. Use whenever asked to "optimise
  my Dockerfile", "make my image smaller", "reduce image size", "my container is
  too big / too slow to build", "speed up my docker build", "shrink the image",
  "clean up my Dockerfile", "multi-stage build for this", "why is my image 1.5GB",
  or "optimise this container image with podman". Trigger even when the user just
  pastes a Dockerfile and asks to make it leaner. This is the OPTIMISATION loop;
  for adding a test suite around the image (runtime/structure/hygiene/capacity)
  use container-testing-rollout instead — the two compose: its tests make a
  stronger oracle for this loop.
---

# Optimise a container image (the guarded loop)

Optimising a Dockerfile is easy to do and easy to get wrong: almost every size
win is also a chance to drop a file the app needs, break a layer the runtime
expects, or change behaviour under a new base image. So we never optimise
blind. We **establish a baseline that provably works, then change one thing at a
time and re-prove it.**

> The image you ship must do exactly what the baseline did — only smaller,
> faster, or safer. Every step is: **measure → change ONE thing → verify →
> keep or revert.**

Everything runs through **podman** (`podman build`, `podman run`). Commands are
drop-in `docker`-compatible; if the project already uses `docker`, keep that
verb but the loop is identical.

## Step 0 — Establish the oracle (do this before touching anything)

The oracle is the question "does it still work?" made executable. You cannot
optimise safely without one. It has three parts, cheapest first:

1. **It builds.** `podman build -t app:base .` succeeds.
2. **It runs.** The container starts and reaches a healthy state — for a service,
   it answers a request; for a CLI, it produces expected output; for a job, it
   exits 0.
3. **It's correct.** A **test command** the project already has passes against
   the running container or the built artifact — e.g. `moon run app:test`,
   `pytest`, `cargo nextest run`, an HTTP smoke check, `--help`.

Ask the user for the test command if it isn't obvious. If the project has none,
the minimum viable oracle is "builds + runs + answers one real request". Write
the oracle down (a shell snippet in the scratchpad) so every iteration runs the
**same** check.

```bash
# Example oracle for an HTTP service — one script, reused every iteration.
podman build -t app:base . || exit 1
cid=$(podman run -d -p 8080:8080 app:base)
sleep 1
curl -fsS localhost:8080/health || { podman logs "$cid"; exit 1; }
podman rm -f "$cid"
```

## Step 1 — Record the baseline

Capture the numbers you intend to improve, so every later claim is measured, not
felt:

```bash
podman build -t app:base .            # note wall-clock build time
podman images app:base --format '{{.Size}}'   # baseline size
podman history app:base --human       # per-layer sizes — this is your target list
```

`podman history` is the single most useful command here: it shows which layer
contributes what. Optimise the biggest layers first; ignore the 2 kB ones.

## Step 2 — The loop (one optimisation per iteration)

Pick the next item from `references/optimisation-catalogue.md` (ordered by
impact-per-risk). Then, for that ONE change:

1. **Change** only that in the Dockerfile.
2. **Rebuild** into a *new* tag: `podman build -t app:try .`.
3. **Run the oracle** against `app:try` — all three parts.
4. **Measure**: `podman images app:try --format '{{.Size}}'` vs baseline.
5. **Decide**:
   - Oracle passes **and** it's smaller/faster → keep it. Re-tag as the new
     baseline (`app:base`) and move to the next catalogue item.
   - Oracle fails → **revert that change**. Note why it failed (often a missing
     runtime file or a dropped build dep) and move on. Do not stack a second
     change on top of a broken one — you'd lose the ability to bisect.
   - Passes but no measurable gain → revert (churn without benefit is not a win).

Never batch two optimisations into one build. When something breaks, one-change
iterations tell you exactly what did it; batched changes force a re-bisect.

Stop when the remaining catalogue items don't apply, or the gains fall below
what's worth the added complexity (a 3 MB saving that costs a fragile hand-copied
`ldd` list usually isn't).

## Step 3 — Report

Show the before/after: base image, final image size, build time, and the ordered
list of changes that stuck (and any you tried and reverted, with the reason).
Leave the Dockerfile as the new baseline; let the user review the diff.

## The optimisation catalogue

The concrete techniques — ordered, with the failure mode each tends to trip and
how the oracle catches it — live in **`references/optimisation-catalogue.md`**.
Read it when you reach Step 2. Highlights, in rough priority order:

1. **Multi-stage build** — compile/install in a fat builder, copy only the
   artifact into a clean final stage. Usually the single biggest win.
2. **Right-size the base** — `slim`/`alpine`/`distroless`/`scratch` for the final
   stage. Biggest risk of a broken oracle (missing libc, shell, certs).
3. **`.dockerignore`** — stop shipping `.git`, `node_modules`, build caches,
   secrets into the build context. Free, speeds every build.
4. **Layer & cache ordering** — copy dependency manifests and install deps
   *before* copying source, so code edits don't bust the dependency cache.
5. **Fewer, leaner `RUN` layers** — combine, and clean package caches *in the
   same layer* (`apt-get … && rm -rf /var/lib/apt/lists/*`).
6. **Drop build-only deps** from the final image; **pin** base tags and versions
   for reproducibility; **run as non-root**.

## Relationship to container-testing-rollout

This skill is the *optimisation* loop; `container-testing-rollout` builds a real
**test suite** around the image (runtime via testcontainers, structure via
container-structure-test, hygiene/CVEs via Trivy, capacity under cgroup limits).
They compose: if that suite exists, use it as this loop's oracle — it's far
stronger than a curl check, and it turns "smaller" into "smaller and still
passing structure + hygiene gates".
