# calque

> A _calque_ is tracing paper you copy a drawing through — and the linguistic
> term for a word-for-word translation from one language into another. This
> pilot does both: it traces a **live, running system** and re-translates it
> into a modern target framework.

`calque` reflects a live system into a spec you can rebuild from. Point it at a
running web app or a running API; it observes the system's _actual behaviour_
and emits a structured **reflection bundle** — golden artefacts that drive
regeneration in a modern target framework.

The live system is the **source of truth**. Original source code, if we have it,
is only cross-reference — useful for naming and intent, never the primary
authority. We reflect what the system _does_, not what its code _says_ it does.

## Two paths, one philosophy

| Path    | Observe                                                            | Golden                                       | Regenerate into |
| ------- | ------------------------------------------------------------------ | -------------------------------------------- | --------------- |
| **UI**  | DOM + accessibility tree + network (incl. WebSocket) + screenshots | characterization tests + visual baselines    | **React**       |
| **API** | OpenAPI spec + sampled request/response pairs                      | OpenAPI as contract + characterization tests | **Axum**        |

Both paths share the same move: **capture observable behaviour as a spec, then
regenerate it in a target framework, and prove parity by replaying the captured
behaviour against the new system.**

- UI path first target: any web app (old R Shiny / Superset dashboards) → React.
- API path first target: any API → Axum.

```
                         ┌─────────────────────────────┐
   live web app ───────▶ │                             │ ──▶ reflection.json
   (Shiny/Superset)      │           calque            │     + HAR + DOM + a11y
                         │                             │     + screenshots
                         │   observe → reflect → emit  │
   live API ───────────▶ │                             │ ──▶ reflection.json
   (REST/OpenAPI)        │                             │     + openapi.json
                         └─────────────────────────────┘     + sampled req/resp
                                       │
                                       ▼
                         skills consume the bundle:
                         reflect-to-react   (UI  → React golden tests + build plan)
                         reflect-to-axum    (API → Axum golden tests + scaffold)
                                       │
                                       ▼
                         parity loop: replay captured behaviour against
                         the regenerated app, diff against the golden.
```

## The reflection bundle

Every capture run produces a self-contained bundle directory. The
`reflection.json` is the primary artefact a skill consumes; the rest is raw
evidence kept alongside it for verification and for visual regression.

**UI bundle** (`out/ui/<run-id>/`) — `✓` produced today, `◦` planned (see Next steps):

```
reflection.json     # ✓ structured spec (the artefact skills consume)
dom/initial.html    # ✓ serialized DOM after settle
screenshots/        # ✓ initial.png, per recipe step, and NN-<slug>.png per crawled route
network.har         # ◦ full network trace with response bodies + WebSocket frames
dom/a11y-tree.json  # ◦ raw accessibility tree dump (folded form already in reflection.json)
```

`reflection.json` (UI) holds today: `meta` (url, detected framework, viewport),
`components` (inventory from the a11y tree), `inputs` (interactive controls),
`network` (observed requests: url/method/status/mime — bodies are a Next step),
`websocket` (frames: direction/opcode/payload preview), `interactions` (per recipe
step: which step → which network/ws delta), and — when `--crawl` is used —
`pages` (one entry per crawled route: `url`, `title`, `screenshot`, `components`,
`inputs`, `network`). `design_tokens` is a Next step.

**API bundle** (`out/api/<run-id>/`) — all produced today:

```
reflection.json     # ✓ meta + endpoints + samples
openapi.json        # ✓ fetched OpenAPI (the structural golden), when found
samples/            # ✓ one file per (endpoint, input-variant): request + response
```

`reflection.json` (API) holds: `meta` (base_url, openapi_source, counts),
`endpoints` (path/method/summary from the spec), `samples` (method/url/request_body/
status/content_type/response_json). Schema synthesis when no OpenAPI exists, and a
`parity/` diff against the rebuild, are Next steps.

## Layout

```
calque/
├── moon.yml                 # layer: application, language: rust
├── Cargo.toml
├── README.md                # this file — the canonical idea record
├── src/
│   ├── main.rs              # CLI dispatch:  calque ui ...  |  calque api ...
│   ├── browser.rs           # chromiumoxide launch + lifecycle + navigation (UI)
│   ├── capture/
│   │   ├── network.rs       # CDP Network: XHR/fetch + WebSocket frames → trace
│   │   ├── dom.rs           # DOM snapshot + accessibility tree
│   │   ├── visual.rs        # screenshots: full page, viewports, per state
│   │   └── style.rs         # computed styles → design tokens
│   ├── recipe.rs            # UI interaction recipes (declarative steps) + replay
│   ├── api/
│   │   ├── openapi.rs       # discover + fetch (or synthesize) the OpenAPI spec
│   │   ├── sample.rs        # behavioural sampling: vary inputs, record req/resp
│   │   └── plan.rs          # sample-plan format
│   ├── detect.rs            # framework heuristics (Shiny ws, Superset /api/v1, …)
│   ├── reflect.rs           # assemble the bundle → reflection.json
│   ├── models.rs            # serde spec types (shared across paths)
│   └── skills.rs            # bundled skills + `calque skills install`
├── skills/                  # the consuming skills, versioned here, embedded in the binary
│   ├── reflect-to-react/SKILL.md
│   └── reflect-to-axum/SKILL.md
├── recipes/example.yaml     # example UI interaction recipe
├── plans/example.yaml       # example API sample plan
└── out/                     # gitignored — capture bundles land here
```

## Install

`calque` is not published — install it from this directory with Cargo:

```bash
cargo install --path .          # builds release → ~/.cargo/bin/calque (on PATH)
calque --help
calque skills install           # drop reflect-to-react + reflect-to-axum into ~/.claude/skills
```

Re-run with `--force` after changing the code (the installed binary is a frozen
build — and it embeds the skills, so reinstall to propagate edited `SKILL.md`).
Remove it with `cargo uninstall calque`. While iterating, skip the install and run
in place from this directory: `cargo run -- ui capture <url> …`.

**Runtime requirement:** the UI path drives Chrome over the DevTools Protocol, so
a Chrome/Chromium install must be present. The API path needs nothing extra.

## CLI

```bash
# UI path — reflect a live dashboard (point at a locally running instance)
calque ui capture <url> [--recipe recipes/r.yaml] [--out out/ui] [--settle 10]
                        [--crawl [--max-pages 20] [--max-depth 2] [--max-per-route 5]]

# API path — reflect a live API (auth, if any, goes in the sample plan)
calque api capture <base-url> [--openapi <url-or-path>] [--plan plans/p.yaml] [--out out/api]

# Skills — install the bundled consuming skills into ~/.claude/skills
calque skills install [--dir <skills-dir>] [--force]
calque skills list

# Parity (planned) — replay a captured bundle against a regenerated app, diff responses
calque parity <bundle-dir> --against <new-base-url>
```

### Crawling routes (capture more than the landing page)

By default `calque ui capture` reflects only the page you point it at. Pass
`--crawl` to follow the app's own same-origin links breadth-first and capture
**each route** — screenshot, accessibility inventory, and the network it triggers
— up to a page/depth budget:

```bash
calque ui capture http://localhost:3000/ --crawl --max-pages 20 --max-depth 2
```

It harvests links the app actually rendered (`<a href>`), so parameterized routes
come with **valid parameters for free** (a rendered `/orders/123` is followed as-is
— `calque` never invents ids). Off-origin links are skipped; each route is visited
once. Each crawled route lands in `reflection.json` under `pages[]` with its own
`NN-<slug>.png` screenshot; the entry page stays at the top level.

One route often appears with many query permutations (e.g. `/table_info?id=1`,
`?id=2`, …). `--max-per-route` (default 5) caps how many of those — sharing one path
— are captured, so a single table view doesn't flood the bundle with near-identical
shots.

**Waiting for the page to load.** Before each screenshot, `calque` waits for the
network to go quiet (no HTTP request in flight for 500 ms) rather than sleeping a
fixed time — so a route's data and components are present in the shot. `--settle`
(default 10) is the **maximum** seconds to wait; fast pages are captured as soon as
they settle, slow ones get up to that long before `calque` captures anyway. (A
persistent WebSocket — e.g. Shiny — doesn't count against idle, so it won't stall
the wait.)

Caveat: this follows real `<a href>` links, so it suits **multi-page apps**. A SPA
that routes via buttons/JS (client-side routing) won't expose its views as anchors
— driving those needs an interaction recipe (an explicit route list is a Next step).

### Logging — seeing what's happening

`calque` logs its progress to the terminal at `info` level by default — start,
OpenAPI discovery, each sampled request with its status, and the bundle write. If
a capture seems to hang, the last line tells you which step it's on (and HTTP
requests now time out rather than waiting forever).

```bash
RUST_LOG=calque=debug calque api capture <base-url> --plan plans/p.yaml   # per-probe / per-request detail
```

> If you see **no** output at all, check whether `RUST_LOG` is already exported in
> your shell — a value that doesn't mention `calque` (e.g. `RUST_LOG=warn`) hides
> these logs. Set `RUST_LOG=calque=info` explicitly to restore them.

### Where to point calque, and auth

**Point the UI path at a locally running instance of the app** — the dashboard
served on `localhost` from a dev or staging build — not a production URL behind a
login. `calque` drives a plain headless browser with no UI-path authentication, so
**if the app is behind auth (SSO, a login form, a session cookie), the capture will
most likely fail or only capture the login screen.** Run the app locally with auth
disabled (or in an already-signed-in session) and reflect that. Getting past
real-world login is an open item — see Next steps.

**The API path does support auth**, because a service token is the simple, common
case there. Put it in the sample plan's `auth` block (shown below). Secrets are
read from the environment, never the plan file, so plans stay safe to commit.

### UI interaction recipe (declarative)

```yaml
name: filter-by-region
steps:
  - { goto: "https://dashboard.example/app" }
  - { wait_for: networkidle }
  - { screenshot: initial }
  - { select: { selector: "#region", value: "EMEA" } }
  - { wait_for: networkidle }
  - { capture_delta: region-emea }
```

### API sample plan (declarative)

The base URL is the CLI argument (`calque api capture <base-url>`), so the plan
carries only the auth, politeness, and per-endpoint variants:

```yaml
name: orders-api
rate_limit_rps: 2 # politeness — never hammer a live system
auth: # optional; every secret is read from the environment, not this file
  bearer_env: API_KEY # → Authorization: Bearer <$API_KEY>
  header_env:
    X-Api-Key: API_KEY # header-name: env-var-name → X-Api-Key: <$API_KEY>
endpoints:
  - path: "/orders/{id}"
    method: GET
    variants: # vary inputs to surface behaviour OpenAPI omits
      - { path_params: { id: 1 } } # happy path
      - { path_params: { id: 999999 } } # 404 shape
      - { path_params: { id: "abc" } } # 400 / validation shape
```

#### Passing an API key

`calque` never takes the key on the command line — it reads it from an environment
variable named in the plan, so the secret stays out of your shell history and the
plan file stays safe to commit. Three steps:

1. **Name the env var in the plan's `auth` block** — pick `bearer_env` (sends
   `Authorization: Bearer <value>`) or a `header_env` entry (sends a header your
   service expects, e.g. `X-Api-Key`). Both name the env var, not the value:

   ```yaml
   auth:
     header_env:
       X-Api-Key: API_KEY # send header "X-Api-Key" with the value of $API_KEY
   ```

2. **Export the key** into that variable:

   ```bash
   export API_KEY="your-real-key"
   ```

3. **Run the capture** — the key is read from the environment and sent on every
   sampled request:

   ```bash
   calque api capture https://api.internal --plan plans/orders.yaml
   ```

If the env var named in the plan isn't set, `calque` stops with a clear error
(`header env var API_KEY not set`) rather than sampling unauthenticated.

## The skill family

The skills are defined in `skills/` (versioned with this pilot) and embedded into
the binary at compile time. Install them into your personal skills directory with
`calque skills install` — it writes `~/.claude/skills/<name>/SKILL.md`, skipping any
that already exist unless you pass `--force`.

- **`reflect-to-react`** — reads a UI bundle, emits golden characterization tests
  per our `frontend-testing-standards` (MSW handlers built from captured
  `data_endpoints`; component/integration tests from `components`; Playwright
  e2e replaying captured `interactions`; visual-regression baselines from
  screenshots), plus a React build plan. Hands off to `migrate-to-react-fastapi`
  for the page-by-page migration.
- **`reflect-to-axum`** — reads an API bundle, emits an Axum scaffold (routes +
  typed handlers + serde models from the OpenAPI schema) and golden
  characterization tests from the sampled request/response pairs, per our
  `rust-testing-standards`.

## Design rationale

- **Live-first, not source-first.** Old dashboards and APIs lie: dead code,
  drifted comments, behaviour that only emerges at runtime. The running system
  is the only honest spec. Source is cross-reference.
- **Characterization tests as the contract.** We don't guess what the rebuild
  should do; we freeze what the original _did_ and make the rebuild satisfy it.
  Captured network payloads become mocks (MSW) or fixtures; captured behaviour
  becomes assertions.
- **CDP for the UI path.** chromiumoxide drives Chrome over the DevTools
  Protocol, which exposes WebSocket frames, the accessibility tree, computed
  styles and screenshots — everything, framework-agnostically. That is why
  "generic first" works: we listen at the protocol layer, below the framework.
- **OpenAPI + sampling for the API path.** The spec gives structure; sampling
  gives behaviour the spec omits (real response shapes, status codes, error and
  validation formats). Together they're a far richer golden than either alone.
- **Parity by replay.** The same captured behaviour that defines the golden also
  verifies the rebuild: replay it against the new app and diff.

## Status

Both reflectors have working v1 captures, verified end to end:

- **UI** — `calque ui capture <url> [--recipe ...]`: generic capture (DOM + a11y
  component inventory + network incl. WebSocket frames + screenshots) on load,
  plus scripted interaction recipes that record per-step network deltas (the
  reactive contract). Framework detection is a heuristic hint, not a hard
  dependency. Try it: `--recipe recipes/example.yaml`.
- **API** — `calque api capture <base-url> [--openapi ...] [--plan ...]`:
  discover or fetch the OpenAPI spec, parse its endpoints, run an authored or
  auto-derived sample plan (rate-limited), and emit the bundle with one captured
  request/response per variant. Try it against the bundled fixture:
  `python3 tests/fixture_api_server.py 8791` then `calque api capture
http://127.0.0.1:8791 --plan plans/example.yaml`.

The two consuming skills exist: **`reflect-to-react`** (UI bundle → golden
tests + React build, delegating to `frontend-testing-standards` and
`migrate-to-react-fastapi`) and **`reflect-to-axum`** (API bundle → Axum scaffold

- golden tests, delegating to `rust-testing-standards`).

## Next steps

The near-term gaps between "working v1" and the full vision, ordered by value.
Each names the file it touches. The broader idea menu is in **Further ideas** below.

1. **Response bodies + HAR** (`src/capture/network.rs`). Today we record request
   metadata (url/method/status/mime) but not response payloads. Call
   `Network.getResponseBody` after `loadingFinished` and write a real `network.har`.
   This is the highest-value gap: the skills turn captured payloads into MSW mocks
   and test fixtures — without bodies they have shapes but no data.
2. **Design tokens** (`src/capture/style.rs` — currently a stub). Evaluate JS in the
   page to read computed styles off the rendered tree, cluster into a token set
   (colours, fonts, spacing), and add `design_tokens` to the UI `reflection.json`.
   The README and `reflect-to-react` already reference it; the capture is missing.
3. **`calque parity` harness** (new `src/parity.rs` + CLI subcommand). Replay a
   captured bundle against a regenerated app and diff responses field-by-field —
   the automated old-vs-new check the skills currently approximate with tests.
4. **OpenAPI schema synthesis** (`src/api/openapi.rs`). When discovery finds no
   spec, synthesize one from the sampled request/response shapes so the API path
   still yields a structural golden.
5. **Raw a11y-tree dump** (`src/capture/dom.rs`). Write `dom/a11y-tree.json`
   alongside the folded `components` — useful when the inventory loses nuance.
6. **Tests for calque itself**. No Rust tests yet (only `tests/fixture_api_server.py`).
   Add unit tests for the pure pieces (recipe parsing, OpenAPI parsing, sample-name
   slugging) per `rust-testing-standards`; the fixture server enables an API-path
   integration test.
7. **UI-path auth** (`src/capture/network.rs` + `src/recipe.rs`). The UI path has
   **no** auth today — point it at a locally running, unauthenticated instance. Real
   webapp auth is the hard open problem: SSO redirects, a **login form** to submit (a
   recipe `fill` step, then `click`), or a **session cookie** to inject
   (`Network.setCookie`). A simple bearer header is rarely enough, which is why auth
   lives on the API path only. (The API path supports bearer + custom headers via the
   plan's `auth` block.)

Framework specialisation (Shiny WebSocket decoding, Superset dashboard-config
fetch) and the larger ambitions live in **Further ideas** below.

## Further ideas / backlog

> Kept here deliberately so nothing we've discussed gets lost. Not commitments —
> a menu to pull from as the pilot proves out.

### UI path

- **Framework specialisation** after the generic base:
  - **R Shiny**: decode the WebSocket protocol — inputs sent as JSON messages,
    outputs (rendered HTML, plot images, data) received back. The clearest map
    of the app's reactive graph.
  - **Superset**: fetch the dashboard config JSON directly — the dashboard
    _is_ a JSON document describing layout + chart specs. Often the whole layout
    spec is one API call away.
- **Active auto-exploration** (the ambitious v2): discover every interactive
  input and exercise it automatically, rather than relying on hand-written
  recipes. Fragile — needs guardrails.
- **Multi-viewport / responsive capture** — screenshot and snapshot at several
  widths to reflect responsive behaviour.
- **Design tokens → theme config** — emit captured colours/fonts/spacing as a
  Tailwind theme or token file the React build can consume directly.
- **Auth/role states** — capture the app as different user roles to reflect
  permission-gated UI.
- **Drift detection** — re-capture over time; diff bundles to detect when the
  live app changed (and when the rebuild has fallen behind it).

### API path

- **OpenAPI discovery** across well-known paths (`/openapi.json`,
  `/swagger.json`, `/v3/api-docs`, `/api-docs`, …).
- **Schema synthesis when there is no OpenAPI** — infer the schema purely from
  sampled request/response pairs, then emit a synthesized OpenAPI as the golden.
- **Smarter input variation** — boundary values, enum coverage, invalid inputs,
  pagination — to surface status codes, error shapes and validation rules.
- **Auth-flow capture** — record token endpoints / login flows so the rebuild
  reproduces them.
- **GraphQL variant** — introspection query _is_ the spec; sample queries for
  behaviour.
- **gRPC variant** — server reflection API as the spec.
- **Replay parity harness** — first-class `calque parity`: run every captured
  request against the new Axum app and diff responses field-by-field.

### Cross-cutting

- **LLM-assisted enrichment** (rig-core, as in the `oracle` pilot) — name
  components, infer intent, group endpoints into resources, suggest React
  component boundaries from the a11y tree.
- **Old-vs-new diff** — point `calque` at both the original and the rebuild and
  diff their reflections to _prove_ migration parity, not just assert it.
- **Bundle as the lingua franca** — keep `reflection.json` stable enough that
  other target frameworks (Vue, SvelteKit; FastAPI, Actix) can plug in later as
  alternative regeneration targets.

## Safety / authorization

API sampling sends real traffic to a live system. Only ever point `calque` at
**systems you own or are explicitly authorized to test** — these are our own
legacy apps being migrated. Sampling is rate-limited and concurrency-capped by
default; keep it polite. Never use `calque` to probe third-party systems.
