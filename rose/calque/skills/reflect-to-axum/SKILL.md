---
name: reflect-to-axum
description: >-
  Turn a calque API reflection bundle (the capture of a live API — its OpenAPI
  spec plus sampled request/response pairs covering happy paths, 404s, and
  validation errors) into an Axum scaffold (typed routes, serde models, handler
  stubs) plus a golden set of characterization tests, so a rebuild can be
  regenerated to match what the live API actually returns. OpenAPI gives the
  structural contract; the samples give the behaviour OpenAPI omits — and they
  are the source of truth. Use this whenever the user has a calque API bundle (a
  `reflection.json` with endpoints/samples alongside openapi.json and samples/)
  and wants to "turn it into Axum", "build the Axum version from the capture",
  "scaffold the Rust API from this OpenAPI", "make golden tests from the sampled
  API", "reflect this API into Axum", or says "reflect-to-axum". Trigger even
  when they just point at a bundle directory and say "rebuild this API in Rust"
  or "generate the Axum routes and tests from this capture". This skill produces
  the scaffold and golden artefacts and delegates the test layers to
  rust-testing-standards. Not for capturing the API (that is the calque tool
  itself) and not for the UI path (use reflect-to-react).
---

# Reflect a calque API bundle → Axum + golden tests

A regeneration conductor. It reads what `calque` observed a **live API** returning
and turns it into two things: an **Axum scaffold** (routes + typed models + handler
stubs) and a **golden set of characterization tests** that pin the observed
responses. It leans on the repo's `rust-testing-standards` for the test layers.

## The one idea that makes this work

OpenAPI tells you the *shape*; the samples tell you the *behaviour*. The samples —
including the `404` and `400` ones — are the source of truth, because they record
what the live service actually did, validation quirks and all. Every captured fact
becomes a test before it becomes a handler:

| Captured in the bundle | Becomes |
|---|---|
| `openapi.json` `paths` → `endpoints[]` | **Axum routes** (`Router::route`) + handler stubs |
| `openapi.json` `components.schemas` | **serde structs** (request/response models) |
| `samples[]` happy path (2xx + `response_json`) | characterization test: request ⇒ this status + this body shape |
| `samples[]` `404` | not-found behaviour + its error body shape |
| `samples[]` `400` | **validation** behaviour + its error body shape |
| `samples[].request_body` / query / path | the exact inputs each test sends |

The unit of work is one captured **sample**, not one OpenAPI path: *this request
produced this status and this body.* That pair is a test.

## The bundle contract (what you read)

A calque API bundle directory holds:

- `reflection.json` — the spec. Fields:
  - `meta`: `base_url`, `captured_at`, `openapi_source?`, `endpoint_count`, `sample_count`
  - `endpoints[]`: `{ path, method, summary? }` — from the OpenAPI spec
  - `samples[]`: `{ name, method, url, request_body?, status, content_type?, response_json?, response_preview? }`
- `openapi.json` — the fetched spec (the structural golden), if one was found
- `samples/<name>.json` — one file per captured request/response pair

If `endpoints` is empty (no OpenAPI was found) the API was sampled blind — work from
`samples[]` alone, and consider synthesizing an OpenAPI from the observed shapes.

## Procedure

1. **Map the surface.** Read `reflection.json`. List routes from `endpoints` (or,
   if empty, infer them from the distinct `(method, path)` in `samples`). Pull
   request/response model shapes from `openapi.json` `components.schemas` and from
   the `response_json` bodies in `samples`.
2. **Scaffold Axum.** Generate: serde structs for the models; a `Router` with one
   `route` per endpoint; a handler stub per route with the correct signature
   (extractors for path/query/body, typed response). Stubs return `todo!()`-shaped
   placeholders — the tests come first.
3. **Write the golden tests** — invoke **rust-testing-standards** and place tests in
   the layers it defines. For each `sample`, one characterization test: build the
   request from `method` + `url` + `request_body`, call the handler (via
   `tower::ServiceExt::oneshot`, no port bound), and assert **the observed `status`
   and the observed `response_json` shape**. Group tests by endpoint; keep the 2xx,
   404, and 400 samples as distinct cases — they encode distinct behaviour. These
   tests are the **acceptance criteria** and exist before the handler bodies.
4. **Implement handlers to satisfy them.** Fill each stub until its endpoint's tests
   pass. The samples define status codes, error shapes, and validation rules — match
   them, don't improve them (parity first; improvements are a later, separate step).
5. **Close the loop / parity.** Run the suite — the characterization tests *are* the
   parity check: they replay the captured behaviour against the rebuild. (When the
   `calque parity` harness lands, it replays the same samples against the running
   service and diffs; until then the test suite is that diff.) A failing test means
   the rebuild is wrong, not the test.

## Boundaries

- **Samples are authority.** When the OpenAPI schema and a sample disagree (the spec
  drifted from the running service), trust the sample — it's what the live API did.
- **Parity before polish.** Reproduce odd validation and error shapes faithfully;
  flag improvements separately rather than silently fixing them in the rebuild.
- **You don't capture here.** Producing bundles (and the rate-limited sampling) is
  the `calque` tool's job; this skill consumes them. For web UIs, use
  **reflect-to-react**.
