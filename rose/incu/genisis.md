# Incu — From In-Container Updates to Immutable Deployments

The original idea behind Incu was a self-updating application that could hot-swap its own code from within a running container. After working through the design, the conclusion was clear: this fights the container model rather than leveraging it. This document captures _why_, and what to do instead.

---

## 1. Why In-Container Updates Fight the Grain

### The Immutable Infrastructure Principle

Containers exist to provide a guarantee: **what you test is exactly what runs in production**. The image is the artifact. If two environments run the same image, they behave identically. In-container updates break this guarantee by mutating the running state after deployment.

### Concrete Risks of In-Container Mutation

**State drift.** The running container diverges from any tracked artifact. If it crashes and the orchestrator restarts it, you get the _old_ version — not the one you updated to. The "current state" exists only in the ephemeral filesystem of one container.

**Non-reproducibility.** You cannot re-derive the running state from source. `docker inspect` tells you the original image, not what the updater changed. Debugging becomes archaeology.

**Build toolchain in production.** The Incu design required pip, npm, git, and compilers available inside the running container to build new versions. This dramatically expands the attack surface and bloats the image with tools that should never ship to production.

**Rollback complexity.** Incu proposed symlink-based version management, a persistent state JSON file, health-check-driven rollback, and a dedicated updater process. All of this already exists at the platform level — the Nimbus platform's image versioning keeps the last N versions, and `create_or_update_service()` handles atomic cutover.

**Single point of failure.** If the updater process fails mid-swap, the container is in a half-updated state that no external system knows about or can recover from.

### What Incu Was Really Trying to Solve

The pain was never that containers exist — it was that **deployments felt slow and manual**. Running `uv run <service> deploy` from a laptop, waiting for the image build, and hoping nothing breaks is friction. But that friction is a pipeline problem, not a container problem. The solution is to automate the pipeline, not bypass the container model.

---

## 2. When Self-Updating IS Appropriate

In-application updates are a legitimate pattern in specific contexts:

| Context                     | Why                                                                                     | Examples                                    |
| --------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------- |
| **Desktop apps**            | Users install once and cannot be expected to re-download manually                       | Electron (Squirrel), Tauri, Sparkle (macOS) |
| **Edge / IoT**              | Physical devices in the field where redeployment means SSH-ing into hundreds of devices | Mender, RAUC, SWUpdate                      |
| **Embedded systems**        | The "container" is the entire OS; there is no orchestrator                              | Firmware OTA updates                        |
| **Air-gapped environments** | No CI/CD pipeline can reach these machines                                              | USB-delivered update bundles                |

**The common thread:** self-updating makes sense when the deployment mechanism itself is genuinely unavailable or impractical. On a managed cloud platform like Nimbus, the deployment mechanism _is_ the product. Using it is cheaper than reinventing it.

---

## 3. Best Practices for Fast, Cheap Deployments

### Immutable Containers

Build once, run everywhere. The same image goes to dev, staging, prod. Environment-specific configuration comes from environment variables and mounted secrets — never baked into the image. This means:

- A bug discovered in production can be reproduced locally by running the exact same image
- Rollback is trivial: redeploy the previous image tag
- No "works on my machine" divergence between environments

### CI/CD Automation

The highest-leverage change is automating `push → build → deploy`:

```
  push to main ──→ build image ──→ deploy to dev/staging
  tag a release ──→ build image ──→ deploy to prod
```

This removes the human from the deploy loop. The pipeline calls the same functions that are currently run manually. A GitHub Actions workflow for this:

```yaml
name: Deploy Service

on:
  push:
    branches: [main]
    paths:
      - "cocoon/data_view/**"
  release:
    types: [published]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install uv
        uses: astral-sh/setup-uv@v4

      - name: Install dependencies
        run: uv sync
        working-directory: cocoon/data_view

      - name: Deploy to staging
        if: github.event_name == 'push'
        run: uv run data_view deploy --target staging
        working-directory: cocoon/data_view
        env:
          API_KEY: ${{ secrets.NIMBUS_API_KEY }}

      - name: Deploy to prod
        if: github.event_name == 'release'
        run: uv run data_view deploy --target prod
        working-directory: cocoon/data_view
        env:
          API_KEY: ${{ secrets.NIMBUS_API_KEY }}
```

### Docker Layer Caching

The biggest time cost in deployments is building the image. Layer caching makes rebuilds near-instant when only source code changes (not dependencies). The internal gold standard is `cocoon/data_view/Dockerfile`:

```dockerfile
# Stage 1: Frontend deps cached separately from source
COPY frontend/package.json frontend/bun.lock ./
RUN bun install --frozen-lockfile
COPY frontend/ ./
RUN bun run build

# Stage 2: Rust deps cached via stub sources
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release          # Caches all dependency crates
RUN rm -rf target/release/.fingerprint/data_view-*
COPY src/ src/                     # Only project code invalidates this layer
RUN cargo build --release          # Rebuilds only the project binary

# Stage 3: Minimal runtime — no build tools
FROM debian:bookworm-slim
COPY --from=rust-builder /build/target/release/data_view ./
```

**Key principle:** order Dockerfile instructions from least-frequently-changed to most-frequently-changed. Dependencies change rarely; source code changes constantly.

For Python services, the equivalent pattern:

```dockerfile
COPY pyproject.toml uv.lock ./
RUN uv sync --locked --no-install-project
COPY src/ src/
RUN uv sync --locked
```

Nimbus's `useCache: True` in `buildDetails` enables Docker layer caching server-side — but it only helps if the layer order is right.

### Multi-Stage Builds

Build tools (compilers, npm, cargo, cmake) should never appear in the production image. Multi-stage builds separate the build environment from the runtime:

- **Builder stage:** has all the tooling, produces the artifact (binary, JS bundle)
- **Runtime stage:** minimal base image (debian-slim, alpine), copies only the artifact

This reduces image size, attack surface, and startup time.

### Blue-Green Deployments via Targets

The `target` parameter in `deploy_service()` already enables this natively:

```python
deploy_service(service_name="my-app", target="staging")  # → my-app-v1-staging
deploy_service(service_name="my-app", target="prod")     # → my-app-v1-prod
```

Workflow:

1. Deploy to `staging` on every push to main
2. Run smoke tests against the staging URL
3. On release tag, deploy to `prod`
4. If something goes wrong: redeploy previous image (kept by `delete_all_but_x_images(keep_last_n=3)`)

### Feature Flags

Feature flags decouple **deployment** (code shipping) from **release** (users seeing the feature). Deploy frequently, release carefully.

Start simple with environment variables:

```python
import os

ENABLE_NEW_MODEL = os.getenv("ENABLE_NEW_MODEL", "false").lower() == "true"

@app.get("/predict")
def predict(input: PredictInput):
    if ENABLE_NEW_MODEL:
        return new_model.predict(input)
    return old_model.predict(input)
```

These can be passed via Nimbus's `build_args` or service environment configuration. For more sophisticated needs (A/B testing, percentage rollouts, user targeting), consider a flag service like Unleash or LaunchDarkly.

### GitOps

Git is the single source of truth for what is deployed. If you want to know what is running in prod, read the Git history — not the Nimbus dashboard. The CI/CD pipeline enforces this: only code that is merged to main (or tagged as a release) gets deployed. No manual `uv run deploy` from laptops.

---

## 4. What This Looks Like in the Nimbus Setup

### What Already Exists

The `ouroboros` module already handles the full deployment lifecycle:

| Step    | Function                                 | What it does                                                               |
| ------- | ---------------------------------------- | -------------------------------------------------------------------------- |
| Package | `get_files_to_include()`                 | Zips source respecting `.dockerignore`                                     |
| Upload  | `create_or_update_image_version()`       | Idempotent — creates new image or adds version to existing                 |
| Build   | `deploy_image()` → poll loop             | Waits for Nimbus to build the Docker image (15s intervals, 10min timeout) |
| Deploy  | `create_or_update_service()`             | Idempotent — POST if new, PATCH if exists                                  |
| Cleanup | `delete_all_but_x_images(keep_last_n=3)` | Keeps last 3 versions for rollback                                         |
| Scaling | `build_service()`                        | `minInstances: 2` for prod, `scaleToZero: True` for non-prod               |

**This is already 80% of a modern deployment pipeline.** The missing 20% is the CI/CD trigger — replacing `uv run <service> deploy` with an automated workflow.

### What to Add

1. **GitHub Actions workflows** per service (see example in Section 3)
2. **Standardised targets**: currently services use `cocoon`, `dev`, `prod` inconsistently — standardise on `dev` / `staging` / `prod`
3. **Health endpoints**: every service should expose `GET /health` for readiness detection
4. **Smoke tests in CI**: after deploying to staging, hit the health endpoint before promoting

---

## 5. Concrete Next Steps

1. **Start with one service.** Pick `cocoon/data_view` (best Dockerfile in the repo). Create a GitHub Actions workflow that deploys on push to main.
2. **Standardise targets.** Audit all `deploy` CLI commands and align on `dev` / `staging` / `prod`.
3. **Promote the data_view Dockerfile pattern.** All services should use multi-stage builds with dependency caching. Audit the Inertia Jinja template to split dependency installation from source copying.
4. **Add `/health` endpoints** to all services. This is what Incu's Phase 2 reinvented — the platform already uses these for readiness.
5. **Introduce feature flags** for the next significant feature. Start with env-var-based flags.
6. **Remove manual deploy from laptops.** Once CI/CD is running, `uv run deploy` should only be used for emergencies, not routine deploys.

---

## 6. What We Kept from Incu

The instinct behind Incu was correct. Deployments should be fast, safe, and low-friction. The specific ideas map cleanly onto industry-standard practices:

| Incu idea                      | Industry equivalent                                        |
| ------------------------------ | ---------------------------------------------------------- |
| Health checks and verification | Platform-level readiness probes + CI smoke tests           |
| Rollback capability            | Image versioning (`keep_last_n=3`) + redeploy previous tag |
| Update state tracking          | CI/CD pipeline logs + Git history                          |
| Observability of updates       | Apocrypha stack (traces, logs, metrics)                    |
| "Click a button to update"     | Push to main triggers automated deploy                     |
| Version detection              | Git tags / GitHub releases                                 |

The mechanism was wrong; the motivation was right.

---

## Prior Art

- **The Twelve-Factor App** — methodology for building SaaS apps, particularly Factor V (Build, Release, Run) and Factor X (Dev/Prod Parity)
- **Immutable Infrastructure** (Chad Fowler, 2013) — the original argument for treating servers as cattle, not pets
- **GitOps** (Weaveworks) — Git as the single source of truth for declarative infrastructure
- **Erlang/OTP hot code reloading** — the inspiration for Incu, but built into a VM designed for it from the ground up. Containers are not that VM.
