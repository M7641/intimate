# The plan is the process memory

A full Shiny/Dash → React+FastAPI migration is too big to hold in one context window.
The window gets **summarised**, the work spans **multiple sessions**, and (when fanned
out) **many agents** touch it — none of which share your in-head state. So the plan does
not live in the conversation. It lives **on disk, as a set of phase documents** that any
agent or any future session reads to re-establish exactly where things stand. Treat these
files the way you'd treat memory: write to them as you learn, read them before you act.

The single `migration_manifest.json` from orchestration.md is the machine-readable
work-list. This document set is the **human-readable narrative around it** — the roadmap,
the decisions, the running log, the punch-lists — split by phase so each file stays small
enough to read in full and so parallel agents write to different files instead of fighting
over one.

## Layout

Create a `migration/` directory at the repo root in the very first step, before inventory:

```
migration/
├── PLAN.md                 living index: roadmap, status board, gate log, open questions
├── manifest.json           the work-list (schema in orchestration.md) — machine-readable
└── phases/
    ├── 01-understand.md     inventory findings, kind-classifications, dead code, R decision
    ├── 02-rebuild.md        scaffold + API-contract decisions, per-item migration log
    └── 03-polish.md         UI/backend quality punch-lists, acceptance notes
```

For a very large migration, add `phases/pages/<page>.md` — one running log per page — and
have `02-rebuild.md` link to them. This keeps each page's detail out of the shared file so
N fan-out agents append to N different files. Skip it for small migrations; don't create
empty per-page files speculatively.

## PLAN.md — the heartbeat

`PLAN.md` is what you read first, every session. Keep it short and current; push detail
down into the phase files. It holds four things:

1. **Roadmap** — the three movements / eleven steps, drafted up front so the shape of the
   whole job is visible from step one. This is the "plan in phases" — written before work
   starts, then checked off.
2. **Status board** — one row per manifest item with its pipeline stage, so resumption is a
   glance. The pipeline (steps 5-8) is a per-item state machine; record where each item is:

   ```markdown
   | Page            | kind    | port | runs | parity | tests | quality | notes        |
   |-----------------|---------|------|------|--------|-------|---------|--------------|
   | sales_overview  | compute | ✅   | ✅   | ✅     | ✅    | ⬜      | cache groupby|
   | cohort_explorer | compute | ✅   | ✅   | ⚠️     | ⬜    | ⬜      | float drift  |
   | nav_sidebar     | layout  | ✅   | ✅   | n/a    | ⬜    | ⬜      |              |
   ```

   `⬜` not started · `🔄` in progress · `✅` done · `⚠️` blocked/divergent · `n/a`.
3. **Gate log** — each human gate (after steps 3, 4, 11) with the date, what was approved,
   and any scope change the user asked for. This is the audit trail of decisions that the
   summary would otherwise lose.
4. **Open questions / risks** — anything unresolved (the R→Python-vs-bridge call, a parity
   divergence you suspect is a latent source bug). Carry these forward explicitly.

## Lifecycle — when to write, when to read

The plan is only memory if it's kept in sync. Bind reads and writes to the protocol:

- **Step 1 (start):** create `migration/`, draft `PLAN.md`'s roadmap, stub the three phase
  files. The roadmap is written *before* inventory so the whole arc is on paper.
- **Steps 1-3 (understand):** inventory findings, dead-code list, and final `kind`
  classifications go into `01-understand.md`; the manifest is written/edited alongside.
- **Gate after step 3:** record the user's classification fixes and scope sign-off in the
  gate log. Update the status board to list every item as `⬜`.
- **Step 4 (scaffold):** write the chosen architecture and the **full API contract** into
  `02-rebuild.md` — this is shared state every later agent reads. Gate, then log approval.
- **Steps 5-8 (pipeline):** each item updates its status-board row as it clears each stage,
  and appends a short log line (or its `pages/<page>.md`) — what endpoint, what parity
  divergences, what fixtures. A fan-out agent's **first action is to read** `PLAN.md` + its
  manifest item + `02-rebuild.md` (for the API contract); its **last action is to write**
  its row + log. That read/write discipline is what makes the fan-out resumable.
- **Steps 9-11 (polish):** quality findings and the acceptance punch-list go into
  `03-polish.md`; final acceptance in the gate log.

## Resuming a migration

Because the state is on disk, picking up a half-done migration — new session, fresh context,
or a different agent — is mechanical:

1. Read `PLAN.md`. The status board tells you what's done and what's next.
2. For the next `⬜`/`🔄`/`⚠️` item, read its manifest entry and `02-rebuild.md` for the
   contract it must satisfy.
3. Continue the pipeline from that item's stage. Never restart completed stages — trust the
   board (and the characterization tests) over re-deriving.

If `PLAN.md` and the actual code ever disagree (a row says `✅ parity` but no test exists),
trust the code, fix the board, and note the correction in the gate log — a lying memory is
worse than none.

## Why multiple documents, not one

A single growing `MIGRATION.md` works for a tiny app, but at scale it has two failure modes
this layout avoids: it grows past what any agent reads in full (so detail is silently
skipped), and it becomes a write-contention point when fanning out (N agents serialising on
one file, or clobbering each other). Splitting by phase — and optionally by page — keeps
every file readable in one pass and lets concurrent agents append to disjoint files. The
cost is one index (`PLAN.md`) to tie them together, which you wanted anyway as the heartbeat.
