# Orchestration — how to fan the migration out

Shared across R and Dash sources. This is the *how to run it at scale* layer: the
manifest that drives everything, the per-item pipeline, parity verification, and
where the per-item work becomes a `Workflow` fan-out.

## The manifest is the spine

Movement 1 produces one artifact that drives the rest: a manifest of work items.
Write it to disk as `migration/manifest.json` — alongside the phase documents that
carry the rest of the process state (see planning.md) — so it survives context resets,
is editable by the user at the gate, and lets the fan-out resume. The manifest is the
machine-readable work-list; `PLAN.md`'s status board tracks each item's pipeline stage.
One item per migratable unit:

```json
{
  "page": "sales_overview",
  "kind": "compute",                         // compute | layout | plumbing | dead
  "source_ref": "app.R:120-180",             // or app.py:callbacks
  "inputs": ["region", "date_range"],        // reactive inputs it depends on
  "outputs": ["revenue_plot", "kpi_table"],  // what it renders/returns
  "target_endpoint": "GET /api/sales/overview",
  "target_components": ["SalesOverview.tsx", "RevenueChart.tsx"],
  "notes": "heavy groupby; candidate for server-side caching"
}
```

`kind` decides the destination: `compute` → a FastAPI endpoint, `layout` → React
components, `plumbing` → reborn as fetch wiring (not ported), `dead` → dropped. The
user edits this at the post-step-3 gate. Everything downstream reads it.

## The per-item pipeline (steps 5-8)

Migrate → runs → parity → tests is **one `pipeline()` per item**, not four barriers.
Item A can be in parity-diff while item B is still being ported — wall-clock is the
slowest single chain, not the sum of slowest-per-stage. Only opt into this when the
user has asked for multi-agent orchestration (the `Workflow` tool needs explicit
opt-in); otherwise drive the pipeline inline, one item at a time.

Skeleton to adapt when fanning out (only the *port* stage edits files, so it gets a
worktree to avoid clobbering the shared scaffold):

```js
export const meta = {
  name: 'migrate-pages',
  description: 'Port each dashboard page into React+FastAPI with run+parity+test gates',
  phases: [{ title: 'Migrate' }],
}
const items = args.filter(i => i.kind !== 'dead' && i.kind !== 'plumbing')  // manifest, passed as args
phase('Migrate')
const out = await pipeline(items,
  it      => agent(`Port ${it.source_ref}: layout → ${it.target_components}, `
                 + `compute → ${it.target_endpoint}, wire with a client fetch keyed on ${it.inputs}.`,
                   { schema: PORT, isolation: 'worktree' }),
  (_, it) => agent(`Run it: does ${it.target_components[0]} render and does ${it.target_endpoint} return?`,
                   { schema: RUNS }),
  (_, it) => agent(`Parity-diff: feed ${it.inputs} to the OLD ${it.source_ref} and the NEW `
                 + `${it.target_endpoint}; list output divergences and fix them.`, { schema: PARITY }),
  (_, it) => agent(`Write pytest characterization tests for ${it.target_endpoint} asserting the OLD outputs.`,
                   { schema: TESTS }),
)
return { migrated: out.filter(Boolean) }
```

Each agent in the fan-out reads `migration/PLAN.md` + its manifest item + the API contract
in `phases/02-rebuild.md` as its first act, and updates its status-board row (and a log line
or `phases/pages/<page>.md`) as its last — that read/write discipline is what makes the
fan-out resumable across context resets. See planning.md.

**When a barrier is right instead.** If you need to dedupe shared components across
pages, or decide the full API surface before any port, collect with `parallel()`
first, then fan out — that genuinely needs all items at once. Don't barrier just to
"flatten a list"; do that inside a pipeline stage.

## Parity verification (step 7) — the heart of trust

The old app is the oracle. Parity means: *same inputs, same outputs*. Make it
mechanical, not eyeballed:

1. **Capture old outputs.** Drive the source app over a grid of representative
   inputs and record outputs (the plot data/JSON, the table rows, the computed
   scalars — not pixels). For Dash, call the callback function directly with input
   values. For Shiny, extract the reactive's compute into a callable, or capture via
   `shinytest2`/headless. Save as fixtures: `inputs → expected_outputs`.
2. **Run the same grid through the new FastAPI endpoint.**
3. **Diff structurally** — numeric tolerance for floats, set-equality for unordered
   rows, exact for the rest. A divergence is a bug in the port (or a latent bug in
   the source you've now surfaced — flag, don't silently "fix" to match).

Those captured fixtures are exactly what step 8's characterization tests assert
against, so capture once and reuse. This is why step 2 never deletes from the
source — you need it runnable as the oracle through step 8.

## Characterization tests (step 8)

The goal is "all the functionality is there," expressed executably. Generate tests
*from the old behavior*, run against the new:

- **Backend** — pytest tests asserting each endpoint returns the captured fixtures.
  Delegate the structure (layout, fixtures, perf gate) to **python-testing-standards**.
- **Frontend** — component tests (renders the right thing for given props/data) and
  an e2e smoke per page. Delegate to **e2e-testing** (Vitest +
  Testing Library + MSW + Playwright).

These are the migration's safety net: they let step 11's tweaks and steps 9-10's
quality passes proceed without fear of regressing parity.

## Why gates, not one autonomous run

A `Workflow` runs start-to-finish with nobody in the loop. This migration has two
points where a wrong upstream decision is catastrophic and cheap to fix early:

- **After step 3** — the manifest's `kind` classifications decide what becomes an
  endpoint vs a component vs dropped. Wrong here = every downstream agent builds the
  wrong thing.
- **After step 4** — the API contract is shared state every port writes against.
  Wrong here = re-porting everything.

So: fan out *within* a movement, gate *between* movements. Run Movement 2's pipeline
only after the user signs off Movement 1's manifest and the scaffold.
