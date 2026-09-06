# Architecture & Project Index

A map of everything in this monorepo. The [`README.md`](README.md) explains the
**tooling** (proto / moon / lefthook — how to build and test); this document explains
the **projects** (what each codename is and why it exists).

> The canonical project graph lives in [`.moon/workspace.yml`](.moon/workspace.yml).
> Run `moon query projects` to see it as the build system sees it. This file is the
> human-readable companion to that machine-readable list.

---

## The map

```
intimate/
├── blank/         Python — foundation libraries (a uv workspace)
│   ├── pure         near-dependency-free utilities (logging, profiling, DAG)
│   ├── ouroboros    platform CLI (services, workflows, images, tenants)
│   ├── destiny      routing & distance service (OSRM + postcodes) — perf-gated pilot
│   ├── inertia      Copier project-template generator (react / dbt / maturin / starlight)
│   └── io/          IO layer: database (PEP-249, multi-warehouse), sthree (S3), sftper (SFTP)
│
├── mana/          Python — embedding-centric ML library (encode → embed → adapt → serve)
│
├── cocoon/        Rust — the production workspace (Cargo). The tako-* crate family + apps.
│   ├── apps/        data_view, warehouse, tako, nyx
│   └── crates/      tako-* (api/parse/schema/database/blobs/keyvalue/test), service-kit, ouroboros
│
├── rose/          Polyglot sandbox — pilots, experiments, and the design documents
│   └── …
│
├── .config/       Tool configs kept off the root (lefthook, cargo-deny, gitleaks, sqruff)
├── workspaces/    Remote dev-environment images (Docker) — simple, desktop, future
├── .claude/       Claude Code config (CLAUDE.md + skills/ + hooks/) — symlinked to ~/.claude/ on push
└── proto-plugins/ Pinned tool manifests (cargo-deny, gitleaks, lefthook, ruff, sqruff, ty)
```

---

## `blank/` — Python foundations

A [uv workspace](reviews/workspaces.md) of the core libraries. `pure` sits at the bottom;
everything builds up from it.

| Project                          | Lang | Purpose                                                                                           | Depends on      | Status                               |
| -------------------------------- | ---- | ------------------------------------------------------------------------------------------------- | --------------- | ------------------------------------ |
| [pure](blank/pure)               | py   | Dependency-free utilities: structured logging, profiling, DAG execution (stdlib + networkx only). | —               | foundation                           |
| [ouroboros](blank/ouroboros)     | py   | CLI for platform — manage services, workflows, images, tenants; deploy containers.           | pure            | active                               |
| [destiny](blank/destiny)         | py   | Routing & distance service over OSRM + postcodes.                                                 | pure, ouroboros | pilot — perf-gated, co-located tests |
| [inertia](blank/inertia)         | py   | Copier-based scaffolder for react / dbt / maturin / starlight projects.                           | —               | tool, stable                         |
| [io/database](blank/io/database) | py   | PEP-249 DB abstraction: Redshift, Snowflake, DuckDB, SQLite; pooling + Jinja2 templating.         | pure, sthree    | core infra                           |
| [io/sthree](blank/io/sthree)     | py   | S3 client over boto3 with Polars integration.                                                     | —               | stable                               |
| [io/sftper](blank/io/sftper)     | py   | SFTP upload/download over Paramiko.                                                               | pure            | placeholder/prototype                |

---

## `mana/` — ML library

| Project      | Lang | Purpose                                                                                                                                        | Depends on     | Status    |
| ------------ | ---- | ---------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | --------- |
| [mana](mana) | py   | Embedding-centric ML: encode features → train contrastive set embeddings (mimic) → adapt to tasks (recommender/forecast) → serve via ANN/ONNX. | pure, database | prototype |

---

## `cocoon/` — Rust production workspace

A single Cargo workspace. The **apps** are deployable services that
compose those crates. Inter-crate dependencies are inferred from `Cargo.toml`.

**Apps** (deployable binaries):

| Project                            | Purpose                                                                                      | Composes                              | Status                |
| ---------------------------------- | -------------------------------------------------------------------------------------------- | ------------------------------------- | --------------------- |
| [data_view](cocoon/apps/data_view) | Snapshot data explorer (supply/demand/reference from Redshift) + React UI. Port 8050.        | tako-database, service-kit, ouroboros | production, Rust-only |
| [warehouse](cocoon/apps/warehouse) | Warehouse analytics & cost explorer (Redshift + Snowflake). Split from data_view. Port 8051. | tako-database, service-kit, ouroboros | production, Rust-only |
| [tako](cocoon/apps/tako)           | Data ingest API: accepts CSV/JSON/Parquet/Avro/Vortex → Parquet → S3 → warehouse. Port 3000. | tako-api                              | production            |
| [nyx](cocoon/apps/nyx)             | LLM-powered dev CLI agents (commits, clarity, Q&A) via Claude or local Candle models.        | rig-core, candle                      | prototype             |

**Crates** (libraries):

| Crate                                        | Purpose                                                                                      | Used by                                     |
| -------------------------------------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------- |
| [tako-api](cocoon/crates/tako-api)           | Router, routes, app state, scheduler for the ingest API.                                     | tako                                        |
| [tako-parse](cocoon/crates/tako-parse)       | Multi-format file parser → Polars DataFrame; bridges Vortex via Arrow IPC.                   | tako-api                                    |
| [tako-schema](cocoon/crates/tako-schema)     | Schema registry, validation, inference (Polars structs + fingerprinting).                    | tako-api, tako-parse                        |
| [tako-database](cocoon/crates/tako-database) | Multi-backend DB abstraction (DuckDB, SQLite, Postgres/Redshift, Snowflake REST).            | tako-api, data_view, warehouse, service-kit |
| [tako-blobs](cocoon/crates/tako-blobs)       | Blob storage abstraction (S3 or local filesystem).                                           | tako-api                                    |
| [tako-keyvalue](cocoon/crates/tako-keyvalue) | Key-value store abstraction (memory; optional fjall).                                        | — (exploratory)                             |
| [tako-test](cocoon/crates/tako-test)         | Test/data-gen utilities (gen-data, test-upload, benchmark).                                  | tako                                        |
| [service-kit](cocoon/crates/service-kit)     | Shared Axum scaffolding: state, errors, middleware, rate limiting, circuit breaker, metrics. | data_view, warehouse                        |
| [ouroboros](cocoon/crates/ouroboros)         | Platform client (read CLI + deploy library) — **Rust port of `blank/ouroboros`**.       | data_view, warehouse                        |

---

## `rose/` — the polyglot sandbox

Where patterns get prototyped before they graduate (or get archived).

| Project | Lang | Purpose |
| --- | --- | --- |
| [apocrypha](rose/apocrypha) | py | Observability stack: Prometheus/Tempo/Loki/Pyroscope → Grafana via OTel. Sub-apps: `prometheus` (deploy), `spawner` (synthetic-load demo). |
| [dragonfly](rose/dragonfly) 📖 | rust+py | WASM plugin architecture: Rust plugin → WASM, loaded by Python host via Extism (validate/transform). |
| [extract](rose/extract) | py | Schema-driven structured extraction from text+image using open-weight VLMs (Qwen2.5-VL, Gemma). |
| [fusion](rose/fusion) 📖 | rust | Embedded analytics: Arrow + DataFusion + Parquet as a zero-serialization ML/analytics substrate. |
| [galicia-rs](rose/galicia-rs) | rust | DuckDB + Apache Iceberg on object storage as a Redshift alternative for team-scale analytics (+ `galicia-py` twin). |
| [glisen](rose/glisen) 📖 | gleam | Supervision trees on BEAM/OTP — fault-tolerant orchestration of concurrent workflows. |
| [gorgonise](rose/gorgonise) | py | Q-learning RL reference on a 4×4 GridWorld (tabular RL "hello world"). |
| [incinerate](rose/incinerate) | rust | Deep-learning exploration with the Burn framework (wgpu GPU backend). |
| [incu](rose/incu) 📖×2 | docs | Deployment philosophy: immutable infra over self-updating containers + a full AWS infra map. |
| [HTTP-ASM64](rose/HTTP-ASM64) | asm | Minimal HTTP server in pure x86-64 assembly via Linux syscalls. |
| [kairos](rose/kairos) 📖 | haskell | Allen's interval algebra + a bi-temporal fact store; laws checked with QuickCheck (the temporal core of `saga`). |
| [lagoon](rose/lagoon) | py | MiniStack pilot: spin up local AWS (S3, RDS Postgres) inside tests and query it. |
| [meridian](rose/meridian) | bash | Atlas pilot: manage DB schema as code — generate/lint/apply migrations from a declarative `schema.sql` (SQLite, no Docker). |
| [moss](rose/moss) | rust | Axum HTTP gateway → tonic gRPC service ("binary inside, text at edge"). |
| [nameless](rose/nameless) | rust | RL "gymnasium" for Rust code optimization with a TUI dashboard (Claude API or local TinyLlama). |
| [nix_stack](rose/nix_stack) 📖 | rust+ts+py | Nix-reproducible polyglot dev environment (Axum + SolidJS + Typer). |
| [perch](rose/perch) | rust+py | Axum-on-Lambda with ~$0/mo idle (CloudFront → Lambda Web Adapter → Axum), Pulumi-deployed. |
| [quartz](rose/quartz) | ts+py | SolidJS + shadcn-solid + TanStack Router pilot (Solid mirror of the React template). |
| [saga](rose/saga) 📖 | py | Context database for agents: bi-temporal episodic+semantic memory, hybrid retrieval (vector+FTS+recency). |
| [siren](rose/siren) | rust | Interactive text-to-image studio: Stable Diffusion via Candle (Metal/CUDA). |
| [troy](rose/troy) 📖 | rust+py | Multi-language contract registry: Protobuf (internal), OpenAPI (edge), JSON Schema (data). |
| [unity](rose/unity) 📖 | rust→wasm | Rust → WebAssembly canvas-rendering pilot (`unity/two` cdylib). |
| [vive](rose/vive) 📖 | rust | Durable execution for long-running workflows via the Restate SDK (`restate_pipeline`). |
| [archive](rose/archive) | mixed | Retired patterns kept for reference (react_minimal, shotgun, common-rs-pattern, rate-limiting & shutdown notes). |

📖 = has a design essay; see [Design essays](#design-essays).

---

## Supporting infrastructure

### `workspaces/` — remote dev environments (Docker)

| Project                       | Purpose                                                                                                                                                                                               |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [simple](workspaces/simple)   | Cleaned-up minimal platform workspace + VS Code tunneling helpers ([TUNNELING.md](workspaces/simple/TUNNELING.md)).                                                                                   |
| [desktop](workspaces/desktop) | Full XFCE desktop over KasmVNC on Kubernetes (code-server, jupyter, ssh, mise).                                                                                                                       |
| [future](workspaces/future)   | Vision for the next-gen workspace: Hyprland + Selkies + Arch on GPU nodes ([VISION.md](workspaces/future/VISION.md), [BEST-IN-CLASS-OPEN-SOURCE.md](workspaces/future/BEST-IN-CLASS-OPEN-SOURCE.md)). |

### `.claude/` — Claude Code config (project config **and** the personal install)

One directory, two jobs. Claude Code reads `.claude/` natively, so this is the config for
*this* repo; the `sync-skills` / `sync-claude-md` pre-push jobs also symlink it into
`~/.claude/`, so the same files load in **every** Claude Code session. It is
version-controlled — only `.claude/settings.local.json` is gitignored.

```
.claude/
  CLAUDE.md            global context, symlinked to ~/.claude/CLAUDE.md
  skills/              house skills, each symlinked to ~/.claude/skills/<name>
  hooks/               lint.sh (PostToolUse formatter), load-agents.sh (loads AGENTS.md)
  settings.json        hook wiring — tracked
  settings.local.json  per-developer permissions — gitignored
```

Because `.claude/CLAUDE.md` is a native project-memory location, that file is read twice
in this repo: once as user context (via the `~/.claude/` symlink) and once as project
context. Harmless at its current size; see the note in `.config/lefthook.yml`.

---

## How to navigate from here

- **"How do I build/test this?"** → [`README.md`](README.md) (proto / moon / lefthook).
- **"What projects exist and how do they relate?"** → this file, or `moon query projects`.
- **"Why was something built this way?"** → the [design essays](#start-here-the-design-essays).
- **"What's affected by my change?"** → `moon query affected`.
