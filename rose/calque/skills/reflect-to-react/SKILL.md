---
name: reflect-to-react
description: >-
  Turn a calque UI reflection bundle (the capture of a live web app — DOM,
  accessibility tree, network incl. WebSocket frames, screenshots, and
  per-interaction deltas) into a golden set of characterization tests plus a
  React build plan, so a rebuild can be regenerated to match what the live app
  actually does. The live capture is the source of truth; original source code,
  if present, is only cross-reference. Use this whenever the user has a calque
  UI bundle (a `reflection.json` with components/inputs/network/interactions
  alongside dom/ and screenshots/) and wants to "turn it into React", "build the
  React version from the capture", "make golden tests from the captured
  dashboard", "reflect this app into React", "rebuild this Shiny/Superset
  dashboard in React from what calque captured", or says "reflect-to-react".
  Trigger even when they just point at a bundle directory and say "rebuild this
  in React" or "generate the spec and tests from this capture". This skill
  produces the golden artefacts and delegates the test layers to
  frontend-testing-standards, then hands the page-by-page build to
  migrate-to-react-fastapi. Not for capturing the app (that is the calque tool
  itself) and not for the API path (use reflect-to-axum).
---

# Reflect a calque UI bundle → React + golden tests

A regeneration conductor. It reads what `calque` observed a **live web app** doing
and turns it into two things: a **golden set of characterization tests** that pin
the observed behaviour, and a **React build** made to satisfy them. It leans on the
repo's `frontend-testing-standards` for the test layers and hands the page-by-page
build to `migrate-to-react-fastapi`.

## The one idea that makes this work

The capture is the spec. We do not guess what the rebuild *should* do — we freeze
what the original *was observed to do* and make React satisfy it. Every captured
fact becomes a test before it becomes a component:

| Captured in the bundle | Becomes |
|---|---|
| `network[]` / `interactions[].network_delta` (REST calls + payloads) | **MSW handlers** + fixtures (the mocked backend) |
| `websocket[]` frames (e.g. Shiny inputs→outputs) | MSW WebSocket handlers + the input→output contract |
| `components[]` (a11y roles + labels) | **React component inventory** + Testing-Library queries (by role/name) |
| `inputs[]` (interactive controls) | controlled components + the events tests drive |
| `interactions[]` (step → network delta) | **Playwright e2e** assertions: this input change ⇒ this request/response |
| `pages[]` (one per crawled route) | **React routes** — a route/page component each, with its own components/inputs/data |
| `screenshots/` (initial + per state + per route) | **visual-regression baselines** |
| `meta.framework` (shiny / superset) | which source-specialisation notes to read |

The unit of work is one captured **interaction**, not one source file: *which input
changed, which request it triggered, which output came back.* That triple is a test.

## The bundle contract (what you read)

A calque UI bundle directory holds:

- `reflection.json` — the spec. Fields:
  - `meta`: `url`, `captured_at`, `framework` (`"shiny"|"superset"|null`), `viewport`
  - `components[]`: `{ role, name? }` — from the accessibility tree
  - `inputs[]`: `{ role, name? }` — interactive controls
  - `network[]`: `{ url, method?, status?, mime_type?, resource_type? }`
  - `websocket[]`: `{ request_id, direction: "sent"|"received", opcode, payload_preview }`
  - `interactions[]`: `{ step, network_delta[], websocket_delta[] }` — the reactive contract
  - `pages[]` (present when the capture was crawled): `{ url, title?, screenshot, components[], inputs[], network[] }` — each is a distinct route; the entry page is the top-level fields above
- `dom/initial.html` — serialized DOM (cross-reference for structure)
- `screenshots/*.png` — `initial`, one per recipe step, and `NN-<slug>.png` per crawled route (visual baselines)

If a field is empty (e.g. no recipe was replayed, so `interactions` is `[]`), say so
and capture more with calque rather than inventing behaviour. When `pages[]` is
present, the app has multiple routes: build a **React route per entry** (entry page
+ one per `pages[]`), each satisfying its own component/data/visual tests, and wire
them under a router.

## Procedure

1. **Map the app.** Read `reflection.json`. Group `components` into candidate React
   components by role/region; list `inputs`; collect every distinct request in
   `network` + `interactions[].network_delta` as the **data endpoints** the app
   depends on. Note `meta.framework`.
2. **Build the mocked backend.** Generate MSW handlers from the data endpoints —
   one handler per observed `(method, url)`, returning the observed response shape
   as a fixture. For `websocket` traffic, generate an MSW WebSocket handler that
   replays the captured input→output frame pairs.
3. **Write the golden tests** — invoke **frontend-testing-standards** and place
   tests in the layers it defines:
   - *component*: for each meaningful `component`/`input`, a Testing-Library test
     querying **by role + accessible name** (taken straight from the a11y capture).
   - *integration*: render against the MSW handlers; assert the rendered output
     matches the captured response.
   - *e2e (Playwright)*: for each `interaction`, drive the input the recipe drove
     and assert the same request fires and the same output appears.
   - *visual*: register the `screenshots/` as baselines.
   These tests are the **acceptance criteria** — they exist before the React code.
4. **Build React to satisfy them.** Hand the page-by-page build to
   **migrate-to-react-fastapi**, passing the golden tests as the parity contract.
   If original source exists, that skill uses it; if not, build greenfield from the
   spec. The live capture stays the source of truth either way.
5. **Close the loop.** Run the suite. Where a test fails, the rebuild — not the
   test — is wrong (the test encodes observed truth). Re-capture with calque only
   when a behaviour was never observed.

## Boundaries

- **Source is cross-reference, never authority.** When `dom/initial.html` or the
  original code disagrees with the live `network`/`interactions`, trust the capture.
- **Don't invent unobserved behaviour.** Empty `interactions` means the dashboard's
  reactivity wasn't exercised — ask for a calque recipe run, don't guess.
- **You don't capture here.** Producing bundles is the `calque` tool's job; this
  skill consumes them. For APIs, use **reflect-to-axum**.
