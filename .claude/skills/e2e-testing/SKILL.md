---
name: e2e-testing
description: >-
  How we test web applications end to end — two crossing views of one suite. The
  TypeScript testing trophy (Vitest + Testing Library + MSW + @playwright/test):
  static/types, unit, component + integration, browser E2E, accessibility,
  visual-regression, perf budgets. And the full-stack tier map for a sampleapp app
  (FastAPI + built frontend + Postgres/Redshift): a Postgres harness, API-only HTTP
  tests, and Playwright webapp E2E in PYTHON (pytest + async_playwright) that drives
  the running app against a seeded database. Use this whenever adding or improving
  tests for a frontend or a full-stack app, setting up a test runner, mocking the
  network, writing component / E2E / browser tests, driving the running app in a
  browser, testing against a real seeded backend, adding an accessibility or
  visual-regression check, gating bundle size or Lighthouse, planning a whole app's
  test strategy, or deciding which kind of test fits a piece of code. Trigger even
  when the user just says "add tests", "test this component", "test this hook",
  "mock the API", "e2e test", "test the page", "test the webapp end to end", "drive
  the real app in a browser", "test against a real database", "check accessibility",
  "catch visual regressions", "test the user journey", or "make this defensible". It
  routes the backing tiers to: postgres-test-harness (DB), fastapi-api-testing (API
  contract), python-testing-standards (Python units). For Python-only units use
  python-testing-standards; for Rust use rust-testing-standards.
---

# End-to-end & full-stack testing

A web app fails in ways backend logic doesn't: a render path, a user interaction, an
accessibility tree, a network round-trip, a layout — and then, when it's all
assembled, in the seams *between* frontend, API, and database. So this skill holds
**two views of one test suite**, and a full suite uses both:

- **The trophy** cuts by *what a unit is written in* — the TypeScript layers, most
  value in the _integration_ middle, on a wide _static_ base, under a thin browser
  cap.
- **The tier map** cuts by *which slice of the running stack* you exercise — a shared
  Postgres harness, the API contract over it, and the whole product in a browser.

They cross rather than compete. The apex where they meet is **browser E2E with
Playwright** — in two flavours: TypeScript `@playwright/test` for the frontend, and
Python `pytest + async_playwright` for the running full-stack app.

The goal, in both views, is the same as the Python and Rust standards: "works well",
"accessible", and "fast" become _numbers a CI gate defends_, not vibes.

## The principles behind it

These are the non-negotiables. Tools change; these don't.

1. **Test behaviour, not implementation.** Drive the UI the way a user does and
   assert on what a user perceives. Never assert on component state, props, instance
   methods, or CSS class names; never shallow-render. _The more your tests resemble
   the way your software is used, the more confidence they give you._
2. **Query by accessibility first.** Find elements by ARIA role, then label, then
   text — `getByRole('button', { name: 'Save' })`, not `getByTestId`. A `data-testid`
   is the _last_ resort. This makes every behaviour test double as an a11y check —
   in the trophy _and_ in the Python webapp E2E.
3. **Mock only at the seam that matters.** In component/integration tests intercept
   HTTP with MSW; never stub `fetch`/axios, never mock your own modules. In full-stack
   tests, don't mock the backend at all below the API contract — use a real seeded
   database. Fake the boundary, never the behaviour under test.
4. **Simulate real users.** `@testing-library/user-event` (not `fireEvent`) fires the
   full event sequence a real interaction produces. Always `await` it.
5. **Deterministic and isolated.** No real clock, no arbitrary `sleep`. Rely on
   auto-waiting / `findBy` / `waitFor`; use fake timers and factory/seeded data. Reset
   the DOM, MSW handlers, and timers between tests; truncate + reseed between
   full-stack tests.
6. **Accessibility is a gate, not a nice-to-have.** Run `axe` on components and pages;
   a violation fails the build like any other assertion.
7. **Types are part of the contract.** `tsc --noEmit` in strict mode is the static
   base (we keep frontend config minimal — no eslint, a single tsconfig). For public,
   generic APIs add type-level tests so a breaking type change fails too.
8. **Real browsers for the top.** jsdom/happy-dom is fine for component and
   integration tests, but E2E and visual-regression run in a real browser via
   Playwright — the only place layout, real CSS, and cross-browser behaviour live.
9. **Budgets, not hopes.** A bundle-size budget and Lighthouse budgets gate the PR,
   exactly as the backend perf-gate does. A regression you can't measure is one you
   can't defend.
10. **Push every assertion to the lowest tier that can prove it.** If a mocked-DB API
    test can prove the contract, don't write a browser test for it. The browser tier is
    the slowest and flakiest — reserve it for what only the assembled product shows.

## View A — the TypeScript trophy

| Layer                       | Question it answers                  | Tool                                            | CI cadence                           |
| --------------------------- | ------------------------------------ | ----------------------------------------------- | ------------------------------------ |
| **Static / types**          | Does it even type-check?             | `tsc --noEmit` (strict) + `expectTypeOf`        | every PR (blocking)                  |
| **Unit (logic)**            | Is the pure logic correct?           | Vitest                                          | every PR (blocking)                  |
| **Component + integration** | Does the UI behave for a user?       | Vitest + Testing Library + MSW                  | every PR (blocking)                  |
| **Accessibility**           | Can everyone use it?                 | `vitest-axe` / `@axe-core/playwright`           | every PR (blocking)                  |
| **End-to-end**              | Does the real app work in a browser? | `@playwright/test`                              | smoke every PR · full nightly        |
| **Visual regression**       | Did the pixels change?               | Playwright screenshots (or Chromatic)           | every PR (components) / nightly      |
| **Perf budgets**            | Is it still small & fast?            | `size-limit` (bundle) · Lighthouse CI (runtime) | bundle every PR · Lighthouse nightly |

Stack at a glance (all **devDependencies** in `package.json`, none are proto tools):
`vitest`, `@testing-library/react` (+ `/dom`, `/user-event`, `/jest-dom`), `msw`,
`@playwright/test`, `vitest-axe`, `@axe-core/playwright`, `size-limit`, `@lhci/cli`.
Use `happy-dom` as the Vitest environment (faster than jsdom; switch to `jsdom` only
on a fidelity gap).

### 1. Static / types

`tsc --noEmit` in `strict` mode is the floor — it runs as the `lint` task (we
deliberately don't run eslint). For a public, generic API (a hook, a typed client, a
utility), assert the _types_ so a breaking change to inference fails CI too. Vitest
has `expectTypeOf` built in — no extra dependency:

```ts
import { expectTypeOf } from "vitest";
import { useCart } from "./useCart";

test("useCart exposes a typed total", () => {
  expectTypeOf(useCart).returns.toHaveProperty("total").toBeNumber();
});
```

### 2. Unit (logic) — Vitest

Pure functions, reducers, formatters, hook logic — no DOM needed. Fast, the bulk of
your raw test count:

```ts
// useCart.test.ts
import { it, expect } from "vitest";
import { cartReducer } from "./useCart";

it("sums line items", () => {
  const state = cartReducer({ items: [] }, { type: "add", sku: "A", price: 50, qty: 2 });
  expect(state.total).toBe(100);
});
```

### 3. Component + integration — Vitest + Testing Library + MSW

This is the fat layer of the trophy. Render a real component tree, drive it with
`user-event`, let data come from MSW, and assert on the accessible output:

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CheckoutForm } from "./CheckoutForm";

it("submits the order and shows confirmation", async () => {
  const user = userEvent.setup();
  render(<CheckoutForm />);

  await user.type(screen.getByRole("textbox", { name: /email/i }), "a@b.com");
  await user.click(screen.getByRole("button", { name: /place order/i }));

  // MSW answers POST /orders; we wait for the user-visible result.
  expect(await screen.findByRole("status")).toHaveTextContent(/order confirmed/i);
});
```

The network is mocked by MSW at the HTTP boundary, configured once in `test/setup.ts`
and reset between tests — see `references/network-mocking.md`. Note what we did _not_
do: no mocking of `fetch`, no checking of component state, no test IDs.

### 4. Accessibility

Two levels, both gating. In component tests, assert no axe violations on the rendered
tree (`vitest-axe`); in E2E, scan the live page (`@axe-core/playwright`). Because you
already query by role, most a11y bugs surface in the query itself — axe catches the
rest (contrast, missing labels, ARIA misuse):

```tsx
import { axe } from "vitest-axe";
it("has no a11y violations", async () => {
  const { container } = render(<CheckoutForm />);
  expect(await axe(container)).toHaveNoViolations();
});
```

Browser-level a11y and Lighthouse's accessibility budget → `references/e2e-visual.md`.

### 5. Browser E2E + visual regression — `@playwright/test`

A _thin_ layer over the critical user journeys (sign-in, checkout, the flow that
earns the money), run in a real browser against the built frontend. Visual regression
piggybacks on the same browser via `toHaveScreenshot`. Full detail, config, and the
screenshot-baseline workflow are in `references/e2e-visual.md`. Keep these few and
deterministic; the full cross-browser matrix runs nightly.

### 6. Perf budgets

Mirror the backend perf-gate philosophy: a committed budget the PR must stay under.

- **Bundle size** — `size-limit` with a per-entry limit in `package.json`; the
  `bundlesize` task fails the PR if a chunk grows past budget.
- **Runtime / a11y / SEO** — Lighthouse CI (`@lhci/cli`) with asserted budgets (LCP,
  TBT, CLS, a11y score). Slower and noisy → run it nightly against a preview deploy,
  ratcheting the floors. See `references/e2e-visual.md`.

## View B — the full-stack tier map

A sampleapp app is three things — a **FastAPI** backend, a **built frontend**, and a
**Postgres/Redshift warehouse**. Testing the assembled product means tiers over one
shared backing service:

```
        ┌─────────────────────────────────────────────┐
        │  Webapp E2E   browser drives the running app  │  ← Python: references/python-webapp-e2e.md
        ├─────────────────────────────────────────────┤
        │  API tests    HTTP contract, no browser       │  ← fastapi-api-testing
        ├─────────────────────────────────────────────┤
   tier 0  Postgres test harness (container + seeders)   ← postgres-test-harness
        ╞═════════════════════════════════════════════╡
        │  Unit / component   pure logic, mocked deps    │  ← the trophy (View A) · python-testing-standards
        └─────────────────────────────────────────────┘
```

| Tier | Question | Tool | Owner |
|---|---|---|---|
| **Unit / component** | Is each piece correct in isolation? | Vitest+TL+MSW · pytest | View A · python-testing-standards |
| **Harness (tier 0)** | *(not tests — the backing DB the tiers above borrow)* | testcontainers + `TableSeeder` | **postgres-test-harness** |
| **API** | Does the HTTP contract hold? | TestClient / httpx | **fastapi-api-testing** |
| **Webapp E2E** | Does the assembled product work for a user? | Playwright (Python) | **this skill** — `references/python-webapp-e2e.md` |

**There is one backing harness, and the upper tiers borrow it.** A single Postgres
container per module session, seeded once through `TableSeeder`, is shared by the API
integration tests _and_ the Python webapp E2E tests. You do not stand up a database
twice — the same move [gateway-pattern] makes in production: the expensive long-lived
thing is created once; the cheap per-test things (a seeder, a client, a page) borrow
it. Provision **tier 0 first** ([postgres-test-harness]) — the tiers above are empty
shells without it.

### The Python webapp E2E tier

This is the one full-stack tier this skill *owns* directly (the harness and API tiers
have their own skills). It's **Python Playwright** (`pytest` + `async_playwright`, not
`@playwright/test`): build the frontend dist, launch the real FastAPI app on a
background thread, point Chromium at it, and drive the running app against the seeded
Postgres. Full detail — the app-in-thread fixture, the `X-Auth-Email` context header,
seeding inside each test, first-render timeouts, `PLAYWRIGHT_HEADED` — is in
`references/python-webapp-e2e.md`, with `templates/webapp-conftest.py` and
`templates/test_webapp.py`.

## File layout — tests next to the code

Two layouts, one per view. Trophy tests co-locate with the unit they cover; browser
E2E and full-stack tests exercise the whole app, so they live apart.

Frontend (the trophy):

```
src/
├── components/Button/
│   ├── Button.tsx
│   ├── Button.test.tsx     # component + integration + a11y + type tests
│   └── Button.stories.tsx  # optional: Storybook / visual-regression source
├── hooks/useCart/
│   ├── useCart.ts
│   └── useCart.test.ts     # unit / logic
└── test/
    ├── setup.ts            # jest-dom + axe matchers, MSW server lifecycle
    └── msw/handlers.ts     # network handlers — the one mocked seam
e2e/checkout.spec.ts        # @playwright/test, real browser
vitest.config.ts · playwright.config.ts
```

Full-stack (the tier map), per module:

```
modules/<module>/tests/
├── conftest.py             # tier 0: container + db_session   → postgres-test-harness
├── api/                    # API-only: HTTP contract           → fastapi-api-testing
│   ├── conftest.py
│   └── test_*.py
└── webapp/                 # browser E2E: the running app       → python-webapp-e2e.md
    ├── conftest.py
    ├── seeds/              # one TableSeeder per table          → postgres-test-harness
    └── test_*.py
```

Vitest discovers `*.test.ts(x)`; keep `*.spec.ts` for Playwright so the two runners
never collect each other's files. (Existing repo modules name the API folder
`backend/` — same role; `api/` is the clearer name for new work.)

## How testing wires into a moon + proto monorepo

Three facts about the toolchain shape every decision here:

1. **moon is the task runner and the CI gate.** CI runs `moon ci`, which diffs against
   the base branch and runs the _affected_ projects' tasks. TS language defaults live
   in `.moon/tasks/typescript.yml` (`lint` = `tsc --noEmit`); project tasks (`test`,
   `e2e`, `bundlesize`) go in the project's `moon.yml`. A task runs in CI unless it
   sets `options.runInCI: false`. Container-backed tasks (API, webapp) depend on
   `start-podman`; declare package deps (`dependsOn:`) so `--affected` re-tests
   dependents of a changed shared package.

2. **proto pins the runtime.** `node`/`bun` (and Python via uv) are pinned in
   `.prototools`; the test libraries are dev-dependencies, not proto tools.
   Playwright's **browsers** are not npm/pip packages — install them as a task dep
   (`playwright install --with-deps`) so CI has them.

3. **The hooks mirror CI.** `lefthook` runs `moon run :lint`/`:test --affected` on
   pre-push, so the fast layers stay green locally before they reach CI.

```yaml
tasks:
  test:                     # the PR gate (Vitest / pytest unit + component + API)
    command: "vitest run"
    toolchain: "node"
    deps: ["~:install"]
    options: { runInCI: true }
  test-watch:               # local only
    command: "vitest"
    toolchain: "node"
    local: true
    options: { runInCI: false }
```

(E2E, bundlesize, and Lighthouse task definitions live in `references/e2e-visual.md`;
the Python webapp task in `references/python-webapp-e2e.md`.)

## What gates a PR vs what runs nightly

The same judgement across both views (each reference restates it for its own tools):

- **Block the PR** with what's _fast and deterministic_: types, unit, component +
  integration, a11y (axe), all API tests (both flavours — a seeded container is still
  seconds), a small **smoke** E2E set, component visual snapshots, and the bundle-size
  budget. All answer yes/no in seconds-to-low-minutes.
- **Run nightly / scheduled** what's _slow, flaky, or open-ended_: the full E2E matrix
  (every journey × browser × viewport), the full visual-regression suite, and
  Lighthouse against a preview deploy. Treat their output as a **ratchet** — fail only
  when a tracked number regresses past its recorded floor, not on first sight of noise.

A PR gate must be a _budget_, not a _hope_: a flow that boots a container, builds a
frontend, and drives five browsers can't give a clean per-PR yes/no, so only its smoke
subset gates.

## Choosing the right kind for a piece of code

Start from what the code under test actually depends on, and push it to the lowest
tier that can prove it:

- **Pure logic** (reducer, formatter, hook calc, util) → unit test; add an
  `expectTypeOf` test if its types are a public contract.
- **A component with behaviour** (form, menu, anything interactive) → component +
  integration test with `user-event` + MSW + an axe assertion. This is the default.
- **A network-dependent flow** → never stub `fetch`; add/extend an MSW handler and
  test loading, success, and error states.
- **An endpoint whose answer is validation / routing / shape** → mocked-DB API test
  (fastapi-api-testing, flavour 1). Fast, no container.
- **An endpoint whose answer _is_ server-side SQL** (overlays, pivots, SCD4 reads) →
  seeded-container API test (fastapi-api-testing, flavour 2). Don't fake the SQL.
- **A whole user journey** (view → edit → save → see the result) → one Playwright
  spec. Frontend-only against a mocked backend → TS `@playwright/test`
  (`references/e2e-visual.md`); the assembled product against a real seeded DB →
  Python webapp E2E (`references/python-webapp-e2e.md`). Keep both thin — the money
  paths only.
- **A component whose _appearance_ must not drift** → visual-regression snapshot.
- **Anything shipped to users** → it has a bundle-size and Lighthouse budget.

## Rolling this out

Don't build everything at once. For a **frontend**: add the devDependencies and the
four config templates, set Vitest's environment to `happy-dom`, wire `test/setup.ts`
(jest-dom + axe + MSW lifecycle), author the MSW handlers (the seam every integration
test depends on — `references/network-mocking.md`), add the moon tasks, and commit the
visual baselines + size-limit budget so the gates have something to compare against.

For a **full-stack module**, provision tier by tier:

1. **Tier 0** — [postgres-test-harness]: root `conftest.py` safety guard, the session
   container + pool fixtures, one `TableSeeder` per table + the registry.
2. **API** — [fastapi-api-testing]: `tests/api/` with mocked-DB tests for shape and
   seeded-container tests for the SQL-dependent endpoints.
3. **Webapp** — `references/python-webapp-e2e.md`: `tests/webapp/` with the
   app-in-thread fixture and a thin set of seeded, query-by-role journeys.
4. **Wire moon**: container-backed tasks depend on `start-podman`; webapp smoke on PR,
   full nightly.

Get tier 0 green, prove one API test and one webapp test against it, then fill out
coverage. Make `moon ci` a required status check so the gates actually block merges.
