---
name: migrate-to-react-fastapi
description: >-
  Migrate an R Shiny or Plotly Dash app into a split React (frontend) + FastAPI
  (backend) application, page by page and function by function, with parity
  verification and characterization tests so nothing silently breaks. The core move
  is cutting the framework's reactive graph along a clean client/server seam: layout
  becomes React components, callback/reactive bodies become FastAPI endpoints, and
  reactive dependencies become client-side data fetching. Use this whenever the user
  wants to "migrate a Shiny app", "port Dash to React", "rewrite our dashboard in
  React and FastAPI", "get off Shiny/Dash", "modernise our R/Python dashboard",
  "move our analytics app to a real frontend", or describes a page-by-page migration
  of a reactive dashboard to a JS frontend + Python API. Trigger even when they just
  say "migrate this dashboard", "convert our Dash app", or paste a Shiny/Dash file
  and ask to move it to React, without naming every step. This skill conducts the
  migration and delegates quality/test phases to the impeccable-stack,
  backend-impeccable, python-testing-standards, and e2e-testing skills.
  Not for greenfield React/FastAPI apps (no source to migrate) and not for migrating
  between two JS frameworks.
---

# Migrate R Shiny / Dash → React + FastAPI

A migration conductor. It runs a fixed protocol over a **reactive dashboard** and
turns it into a **split app**, leaning on the repo's other skills for the quality
and testing phases instead of reinventing them.

> **Pick the source reference first.** R Shiny and Dash differ in one decisive way —
> R needs a logic-translation decision (R→Python or a service bridge), Dash doesn't.
> Read [references/from-r.md](references/from-r.md) **or**
> [references/from-dash.md](references/from-dash.md) before migrating any page.
> The orchestration (how to fan the work out) is shared:
> [references/orchestration.md](references/orchestration.md), and the on-disk plan that
> carries process state across context resets and sessions:
> [references/planning.md](references/planning.md).

## The one idea that makes this work

Shiny and Dash fuse UI and server compute into a single reactive process. React +
FastAPI splits them. So the migration is not a line-by-line port — it's **cutting
the reactive graph along the client/server seam**:

| Reactive monolith (source) | Split app (target) |
|---|---|
| UI layout / widgets (`ui`, `layout`, `dcc`/`shiny` inputs) | **React components** |
| Reactive/callback **body** (the compute) | **FastAPI endpoint** — a pure fn of inputs → outputs |
| Reactive dependency graph (what re-runs when) | **Client data fetching** (React Query/SWR keyed on UI state) |
| Server-side reactive state (`reactiveVal`, `dcc.Store`) | React client state, or persisted/session state in FastAPI |

Every page you migrate, ask: *what is the compute (→ FastAPI), what is the layout
(→ React), and what triggers the recompute (→ a fetch keyed on which inputs)?* That
question, not the source line count, is the unit of work.

## The plan is the process memory

A migration this size **will not fit in one context window** — the window gets
summarised, the work spans sessions, and a fan-out spreads it across many agents. So the
state of the migration does not live in the conversation; it lives on disk as a **phased
set of plan documents** under `migration/`, and they are the durable memory every session
and every agent reads to know where things stand.

Before inventory, create the plan: an index (`migration/PLAN.md`) holding the roadmap, a
per-item **status board**, and the gate log; one document per movement under
`migration/phases/`; and the machine-readable `manifest.json`. Draft the roadmap **up
front** (the whole arc on paper from step one), then let each phase document accrue detail
as that movement runs. The rule that makes it work: **write to the plan as you learn, read
it before you act** — a fan-out agent's first action is to read `PLAN.md` + its manifest
item, its last is to update its status row. Full document spec, status model, and the
resume procedure are in [references/planning.md](references/planning.md).

## The protocol

Eleven steps in three movements, with **human gates** between movements — a bad
inventory or scaffold poisons every downstream migration, so you stop and let the
user confirm before fanning out. The plan documents are created in step 1 and updated
at every step and gate; treat "update the plan" as part of each step, not a chore.

### Movement 1 — Understand (steps 1-3) · mostly automated, one gate

1. **Inventory** every active surface: pages/tabs, each reactive/callback, each
   widget, each data source. First create `migration/` and draft `PLAN.md`'s roadmap +
   phase stubs (planning.md); then produce the manifest (schema in orchestration.md) that
   becomes the work-list, recording findings in `phases/01-understand.md`. Fan the
   inventory out (read-only) per surface.
2. **Mark dead code** — reactives/outputs nothing depends on, unused branches. You
   do **not** delete from the source; you classify so it isn't carried across.
   Migration is additive into the new scaffold; "removal" is omission, and the old
   app stays intact as the parity oracle.
3. **Separate business logic from plumbing.** The *compute* inside callbacks moves
   to FastAPI; the reactive wiring and layout glue are reborn as React + HTTP, not
   ported. Tag each manifest item: `compute` (→API) / `layout` (→React) / `plumbing`
   (→reborn) / `dead` (→drop).
   - **GATE:** present the manifest. Let the user correct classifications and confirm
     scope before any code is written. This is the cheapest place to fix a mistake.
     Record the sign-off and any scope change in `PLAN.md`'s gate log, and seed the
     status board with every item as `⬜`.

### Movement 2 — Rebuild (steps 4-8) · automated fan-out, gated scaffold

4. **Scaffold** the target: a Vite React app + a FastAPI app. The **API contract is
   the spine** — sketch the endpoints the manifest implies before filling them in.
   The scaffold is shared state every later step writes into, so it's a single
   coherent decision: draft it, then **GATE** — user approves the architecture and
   the API shape before the fan-out. Write the approved architecture and the **full API
   contract** into `phases/02-rebuild.md`; every port agent reads it from there.
5. **Migrate page by page, function by function.** Per manifest page: layout →
   React components, each callback/reactive body → a FastAPI endpoint, wired with a
   client fetch. This is the big fan-out — see orchestration.md.
6. **Make each one run** — the page renders, the endpoint returns. This is a *gate
   per item*, not a separate phase: it's a stage in the same pipeline as step 5.
7. **Parity-diff old vs new.** Feed identical inputs to the old app's compute and the
   new endpoint; diff outputs; fix divergences. The old app is the oracle — this is
   why step 2 never deletes from it. Also a pipeline stage.
8. **Characterization tests** ("PyWrite" = pytest): generate tests from the *old*
   behavior and run them against the *new* endpoints — they become the executable
   definition of "all the functionality is there."
   - Backend tests → delegate to **python-testing-standards**.
   - Frontend component/e2e tests → delegate to **e2e-testing**.

Steps 5→6→7→8 run as **one pipeline per item**, not four barriers — page A can be in
parity-diff while page B is still being ported. No wasted wall-clock. See
orchestration.md for the script shape. Each item updates its `PLAN.md` status row as it
clears each stage, so a resumed session sees exactly which stage every page is at.

### Movement 3 — Polish (steps 9-11) · delegated quality, human acceptance

9. **Quality-pass the UI.** Run **impeccable-stack** on every React component (audit →
   action → critique → action → polish). That skill already chains the impeccable
   calls autonomously — fan it out per component.
10. **Quality-pass the backend.** Run **backend-impeccable** on every FastAPI module
    (API contracts, error handling, transaction boundaries, idempotency, security,
    observability). *(This skill is forthcoming — if it isn't installed yet, do a
    manual backend review against those dimensions and note the gap.)*
11. **Tweak to standard.** The irreducibly human loop: walk each page, compare to the
    old app, fix what's not up to scratch. The conductor surfaces a punch-list (in
    `phases/03-polish.md`); the user drives. **Final GATE / acceptance** — record it in
    the gate log.

## Delegation map

This skill's value is sequencing — each phase hands off to a specialist:

| Phase | Delegates to |
|---|---|
| 8 backend tests | `python-testing-standards` |
| 8 frontend tests | `e2e-testing` |
| 9 UI quality | `impeccable-stack` |
| 10 backend quality | `backend-impeccable` (forthcoming) |

If a delegate skill isn't present, do the work inline against that skill's standard
and flag it — don't silently skip the phase.

## Guardrails

- **Never delete from the source app** until the new app passes parity + tests. The
  old app is the oracle the whole protocol leans on.
- **Stop at the gates.** Inventory (after step 3), scaffold/API (after step 4), and
  acceptance (step 11) are human decisions. Fanning out 50 migration agents on a bad
  manifest is the expensive failure mode.
- **One pipeline, not four barriers** for steps 5-8 — don't synchronize phases that
  don't need cross-item context (orchestration.md explains when a barrier *is* right).
- **Migrate the compute, rebirth the wiring.** Porting reactive plumbing line-by-line
  into React produces a React app that fights React. Cut at the seam.
- For **R sources**, settle the R→Python-vs-bridge decision *before* step 5 — it
  changes every endpoint. See from-r.md.
- **Keep the plan in sync, and trust it over your memory.** The `migration/` docs are
  the process memory across summarisation, sessions, and agents — stale state there is
  the silent failure mode at scale. Update the status row as each stage clears; on resume,
  drive from the board. If the board and the code disagree, the code wins — fix the board
  and note it. See planning.md.
