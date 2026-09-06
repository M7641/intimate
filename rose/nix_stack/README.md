# nix_stack

Nix-reproducible multi-language development environment: Rust (Axum) backend, SolidJS (Vite) frontend, Python (Typer) CLI orchestrator.

Two separate concerns, deliberately kept apart:

- **Development** uses Nix (`flake.nix`) as the single source of truth for the
  toolchain — `nix develop` gives everyone the same rustc, node, python, uv.
- **Production** is a minimal `scratch` container built from the `Dockerfile` —
  a statically-linked Rust binary plus the built frontend, ~3.4 MB, and nothing
  else. It does not use Nix.

Rather than forcing the dev environment to mirror prod byte-for-byte, we do the
reverse: **run the prod artifact locally before deploying** (see [Running
production locally](#running-production-locally)). The container you test is the
container you ship.

## One-time machine setup

```bash
curl -L https://nixos.org/nix/install | sh
# Restart shell
nix flake lock
```

you activate the environment explicitly with `nix develop`.

> **Note on `flake.lock`:** the lock file pins nixpkgs to an exact commit and is
> what makes the environment byte-identical everywhere. If it is missing, run
> `nix flake lock` once and **commit it** — it must be tracked in git.

## Daily use

Enter the dev shell, then run the CLI. The shell's `shellHook` syncs project
dependencies (`uv sync` + `npm install`) on entry, so the code is runnable
immediately — no manual install step.

```bash
cd rose/nix_stack
nix develop              # activates rustc, cargo, node, npm, python, uv
uv run nix-stack dev     # backend :3000 + frontend :5173
```

`nix develop` drops you into a subshell; type `exit` to leave it. To verify the
toolchain inside the shell:

```bash
uv run nix-stack check   # prints versions of rustc, node, python, uv
```

### One-command shortcut

`nix develop -c <cmd>` runs a command inside the dev shell and returns — no
subshell to stay in. Add an alias so a single word brings everything up:

```bash
# in ~/.zshrc or ~/.bashrc
alias nsdev='nix develop ~/Code/nimbus-monorepo/rose/nix_stack -c uv run nix-stack dev'
```

Then `nsdev` from anywhere starts the full stack in the reproducible environment.

## Running production locally

The `Dockerfile` produces the exact image that gets deployed. Building and
running it locally is how you see the real production artifact before it ever
leaves your machine — same binary, same static assets, same `scratch` runtime.
`nix develop` is for _writing_ the code; this is for _shipping_ it.

`podman` and `docker` are interchangeable below — use whichever you have.

```bash
cd rose/nix_stack

# 1. Build the image. Multi-stage: Vite builds the frontend, rust:alpine
#    builds a static musl binary, then both land in a scratch image.
podman build -t nix-stack:prod -f Dockerfile .

# 2. Run it. --rm cleans up on exit; -p maps the container's 3000 to yours.
podman run --rm -p 3000:3000 nix-stack:prod

# 3. See the app — open http://localhost:3000 in a browser, or:
curl http://localhost:3000/api/health   # -> 200
curl http://localhost:3000/api/info     # -> JSON
curl http://localhost:3000/             # -> the built frontend (index.html)
```

The image is ~3.4 MB and runs as a non-root user (`1000:1000`). It contains
only two things — the binary at `/nix-stack-backend` and the frontend at
`/dist` (served via `STATIC_DIR`). There is no shell inside, so debugging is
done by reading logs (`podman logs <container>`) rather than `exec`-ing in.

> **Why `scratch`?** The Rust binary is statically linked (musl), so it needs no
> libc or dynamic loader at runtime — nothing to base the image on. The
> trade-off: if the backend ever makes outbound HTTPS calls it will need CA
> certificates, at which point switch the final stage to
> `gcr.io/distroless/static` (still tiny, but ships certs).

### Testing the image (moon)

We test the image along **three axes** — each answers a different question with
the tool native to it, rather than forcing everything through shell + curl. One
command runs all three:

```bash
moon run nix_stack:image-check
```

| Task (`moon run nix_stack:…`) | Axis | Tool | What it checks |
| --- | --- | --- | --- |
| `image-test` | runtime | testcontainers (Rust) | starts the real container, asserts `/api/health`, `/api/info`, `/` return `200` |
| `image-structure` | structure | container-structure-test | binary + `dist/` present; user `1000`, port `3000`, entrypoint, `STATIC_DIR` |
| `image-scan` | hygiene | Trivy | fails on any HIGH/CRITICAL CVE (scratch → near-zero surface) |
| `image-check` | — | — | aggregate: runs all three |

All depend on `image-build`, so moon builds the `scratch` image first. Notes:

- Each task is defined inline in `moon.yml` (moon's `script:` field) — there are
  no wrapper shell files to maintain.
- **`image-test`** drives the image over HTTP (`backend/tests/image_it.rs`);
  testcontainers removes the container on drop — no manual teardown. It skips
  cleanly when no container engine is reachable. The task points testcontainers
  at Podman's socket and disables ryuk.
- **`image-structure`** and **`image-scan`** run proto-pinned host binaries
  (container-structure-test, Trivy) against a saved tarball (`podman save`), so
  no daemon socket is mounted. Here podman only builds and exports the image.

Just build, no tests: `moon run nix_stack:image-build`.

### Inspecting the image

```bash
podman images nix-stack:prod              # confirm the size
podman run --rm -d -p 3000:3000 nix-stack:prod   # run detached, prints an ID
podman logs -f <id>                       # follow logs
podman rm -f <id>                         # stop and remove
```

## CLI Commands

| Command                           | Description                                       |
| --------------------------------- | ------------------------------------------------- |
| `uv run nix-stack check`          | Verify all required tools and print versions      |
| `uv run nix-stack dev`            | Start backend + frontend dev servers              |
| `uv run nix-stack dev --backend`  | Start only the Rust backend                       |
| `uv run nix-stack dev --frontend` | Start only the Vite dev server                    |
| `uv run nix-stack build`          | Build release binary + production frontend bundle |

## Architecture

Development and production are two distinct environments with two distinct
tools — Nix pins the _toolchain_ you build with; the Dockerfile defines the
_artifact_ you ship. Both are built from the same source tree.

```
  DEVELOPMENT (Nix)                    PRODUCTION (Dockerfile → scratch)
  how you build the code               what actually runs

┌──────────────────────┐            ┌──────────────────────────────┐
│  nix develop         │            │  multi-stage build           │
│  ┌────────────────┐  │            │  ┌────────────────────────┐  │
│  │ rustc          │  │            │  │ node   → dist/         │  │  build
│  │ node           │  │  same      │  │ rust   → static binary │  │  stages
│  │ python, uv     │  │  source    │  └───────────┬────────────┘  │  (thrown
│  └────────────────┘  │  tree      │              ▼               │   away)
│  live reload,        │ ─────────► │  FROM scratch  (~3.4 MB)     │
│  :3000 + :5173       │            │  ├─ /nix-stack-backend       │
└──────────────────────┘            │  └─ /dist  (STATIC_DIR)      │
                                    │  :3000, non-root 1000:1000   │
                                    └──────────────────────────────┘
```

You validate the right-hand side locally with [Running production
locally](#running-production-locally) before it ships — the container you test
is the container you deploy.
