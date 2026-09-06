# Installed skills — inventory

A catalogue of the agent skills currently installed, so you can remember what you
already have before reaching for something new (or building it).

## Where skills live

| Location | What it holds |
|---|---|
| `intimate/.claude/skills/` | **Your house skills** — bespoke, stack-specific, version-controlled in this repo. Symlinked into `~/.claude/skills/` by the `sync-skills` pre-push job. |
| `~/.claude/skills/` | Your personal install. Superset: your house skills **plus** general/marketplace skills. |
| `~/.agents/skills/` | The marketplace/plugin source the personal install draws from. |

To make a new one or improve an existing one, use **`skill-creator`** (see below) — that's
the "skill-making skill" you were looking for.

---

## House skills (authored for this stack)

These encode *our* conventions — the `cocoon`/`rose` monorepos, the
proto + moon toolchain, our testing standards. Prefer these over the generic equivalents
when working in this repo.

### Testing standards (one per language — cut by language, "what kind of test")
- **rust-testing-standards** — Cargo + moon: the five test kinds (unit, benchmark/perf-gate, property, mutation, fuzz), `cargo-nextest`, co-located layout, CI wiring. Covers testcontainers (Postgres / S3Mock).
- **python-testing-standards** — uv + pytest + moon: same five kinds, Rust-style co-located layout, perf gates, integration tests against disposable seeded Postgres / mocked S3.

### Frontend & full-stack application testing (two crossing views — "what kind" for TS, "where in the app" for the stack)
- **e2e-testing** ⭐ — the **frontend + full-stack testing skill** (merges the old frontend-testing-standards, fullstack-testing, and playwright-webapp-testing). View A: the TypeScript testing trophy (Vitest + Testing Library + MSW + `@playwright/test`, a11y/visual/perf budgets). View B: the full-stack tier map (the `tests/{api,webapp}/` layout, borrow-one-harness, PR-vs-nightly). Owns the browser E2E apex in both flavours — TS `@playwright/test` for the frontend, **Python** `async_playwright` for the running app. Start here for frontend or whole-app strategy.
- **postgres-test-harness** — tier 0: spin up a disposable Postgres container per session, Redshift compat layer, three-layer production-safety guard, and seed tables via a generic `TableSeeder` (registry + autouse truncate + realistic-row builders). Both upper tiers borrow it.
- **fastapi-api-testing** — API-only tests: mocked-DB (TestClient + `AsyncMock`) for shape/routing/auth, seeded-container (httpx + ASGITransport) for real server-side SQL. Assert the contract, not internals.

### Rust services
- **axum-production-patterns** — build production-grade Axum (0.8) services: router/state, central `IntoResponse` error type, the middleware stack, observability, pooled DB + circuit breaker, graceful shutdown, probes, OpenAPI, tests. Reuses `service-kit` inside cocoon.
- **reflect-to-axum** — turn a `calque` API reflection bundle (OpenAPI + sampled request/response pairs) into an Axum scaffold plus golden characterization tests.

### Frontend & migration
- **solidjs-patterns** — diagnose/fix SolidJS reactivity bugs (stale signals, lost prop reactivity, mis-timed effects). Failure-mode → fix index.
- **reflect-to-react** — turn a `calque` UI reflection bundle (DOM, a11y tree, network, screenshots) into golden tests + a React build plan.
- **migrate-to-react-fastapi** — migrate an R Shiny / Plotly Dash app into split React + FastAPI, page by page, with parity verification.
- **impeccable-stack** — autopilot the full `impeccable` UI-improvement loop (audit → fix → critique → fix → polish) on a target, minimal supervision.

### Platform & tooling
- **moon-proto-rollout** — roll out the proto + moon + lefthook + Renovate polyglot-monorepo pattern onto another repo (Rust/Python/TS).
- **container-testing-rollout** — roll out three-axes container-image testing (runtime via testcontainers, structure via container-structure-test, hygiene via Trivy) as moon tasks that depend on an image-build; proto-pins the two image-native tools. Rust (scratch) + Python (distroless/slim).
- **bootstrap-ouroboros** — regenerate the `ouroboros` Nimbus platform deploy client from scratch in any directory, only the tiers you ask for.

### Data / warehouse
- **scd4-history** — build a Slowly Changing Dimension Type 4 (history-table) process for an editable dimension table; Redshift-safe SQL (no MERGE/triggers).
- **gateway-pattern** — our data-access convention: a bound `(db, schema)` class over `DBActionsProtocol` managing one `TableArtifact`. The three table roles (`upstream`/`owned`/`event_log`), the `<name>_gateway/` folder layout, atomic multi-gateway transactions, and what belongs in the FastAPI `lifespan` vs per-request.

---

## Meta: making & managing skills and config

- **skill-creator** ⭐ — **the skill-making skill.** Create a new skill from scratch, edit/improve an existing one, run evals to test it, benchmark with variance analysis, and optimize its description for reliable triggering. Start here whenever you want a new skill.
- **find-skills** — discover and install skills when you want functionality you might not have yet.
- **health** — audit the full Claude Code config stack when Claude ignores instructions, hooks misbehave, or MCP servers need checking.
- **dispatching-parallel-agents** — guidance for splitting 2+ independent tasks across parallel agents.
- **zoom-out** — ask the agent to step back and give higher-level context on an unfamiliar area of code.

---

## General-purpose skills (marketplace / third-party)

### Planning & process
- **think** — turn a rough idea into an approved, validated plan before writing code.
- **grill-me** — get interviewed relentlessly about a plan/design until every branch is resolved.
- **learn** — six-phase research workflow turning unfamiliar domains or sources into publish-ready output.

### Code review & quality
- **check** — review a diff after implementation, auto-fix safe issues, run security/architecture reviewers on large diffs; also triage issues/PRs.
- **hunt** — find the root cause of an error/crash/failing test *before* fixing.
- **code-simplifier** — refine recently-changed code for clarity and maintainability without altering behaviour.
- **improve-codebase-architecture** — find deepening/refactoring opportunities, informed by `CONTEXT.md` and `docs/adr/`.
- **rust-best-practices** — idiomatic Rust guidance (Apollo GraphQL handbook): ownership, error handling, performance.
- **sql-code-review** — security/maintainability/anti-pattern review across SQL dialects.
- **documentation-writer** — Diátaxis-framework technical writing.
- **docstring** — document a Python module + classes in Google style.
- **git-commit** — conventional-commit message generation with intelligent staging.

### Python
- **async-python-patterns** — asyncio / concurrency / async-await for I/O-bound systems.
- **fastapi-python** — FastAPI best practices for APIs and async ops.
- **python-performance-optimization** — profile and optimize slow Python (cProfile, memory profilers).
- **python-testing-patterns** — generic pytest/fixtures/mocking/TDD (the non-house version; prefer `python-testing-standards` here).

### Frontend & design
- **design** — distinctive production-grade UI; handles screenshot-driven iteration.
- **impeccable** — single-step UI design/critique/audit/polish (the engine `impeccable-stack` orchestrates).
- **ui-ux-pro-max** — large UI/UX library: styles, palettes, font pairings, product types, chart types across many stacks.
- **industrial-brutalist-ui** — a specific aesthetic: Swiss-print × military-terminal, for data-heavy dashboards.
- **web-design-guidelines** — review UI code against Web Interface Guidelines (a11y, UX).
- **performance** — web performance (load time, page speed) optimization.

### Content & web
- **read** — fetch any URL/PDF as clean Markdown (handles paywalls, JS-heavy pages, X/Twitter). Prefer over WebFetch.
- **write** — strip AI writing tells; rewrite prose to sound natural (EN/ZH).

### Cloud
- **cost-optimization** — reduce cloud spend across AWS/Azure/GCP/OCI (rightsizing, tagging, reserved instances).

---

## Quick "which one?" pointers

- New skill, or improve/test an existing one → **skill-creator**
- "What can I do X with — is there a skill?" → **find-skills**
- Build a Rust web service → **axum-production-patterns**; rebuild a captured one → **reflect-to-axum**
- Add tests → **rust- / python-testing-standards** or **e2e-testing** (frontend/full-stack) (house) before the generic patterns ones
- Test a *whole* web app (strategy, all tiers) → **e2e-testing** (frontend trophy + the tier index), then **postgres-test-harness** → **fastapi-api-testing** → back to **e2e-testing** (Python webapp E2E)
- Just the test DB / seed data → **postgres-test-harness**; just API endpoints → **fastapi-api-testing**; just browser E2E → **e2e-testing**
- Plan before coding → **think**; stress-test the plan → **grill-me**
- Something's broken → **hunt** (root cause) then **check** (review the fix)
- Set up a new repo's tooling → **moon-proto-rollout**; then test the image it ships → **container-testing-rollout**
