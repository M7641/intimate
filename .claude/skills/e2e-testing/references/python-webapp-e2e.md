# Webapp E2E in Python (pytest + async_playwright)

Browser end-to-end tests of the **running application** — built frontend served by
the real FastAPI app, talking to a real seeded Postgres. This is the top tier of
the full-stack map: the only place the whole assembled stack is exercised the way a
user hits it.

**This is Python Playwright** (`pytest` + `async_playwright`), not the TypeScript
`@playwright/test` runner used for frontend component/E2E. They coexist on purpose:

| | Python webapp E2E (this file) | TS E2E (`references/e2e-visual.md`) |
|---|---|---|
| Runner | `pytest` + `async_playwright` | `@playwright/test` |
| Under test | the **running app** (built dist + real API + seeded DB) | the **frontend code** (components, hooks) |
| Backend | real, seeded ([postgres-test-harness]) | mocked at the network boundary (MSW) |
| Lives in | `modules/<m>/tests/webapp/` | `e2e/*.spec.ts` next to the frontend |

Reach here when the question is "does the assembled product work"; reach for the TS
E2E layer when it's "does this component behave".

## Folder layout

```
modules/<module>/tests/
├── conftest.py          # postgres-test-harness: container + db_session
└── webapp/
    ├── conftest.py      # backend_url (app-in-thread) + seeder fixtures
    ├── seeds/           # one TableSeeder per table (see postgres-test-harness)
    └── test_*.py        # the Playwright specs
```

The shared browser fixtures (`browser`, `context`, `page`) live once in a
`playwright_fixtures` plugin registered from the **root** `conftest.py`
(`pytest_plugins = ["common_py.testing.playwright_fixtures"]`), so every module's
webapp tests get them for free.

## Bringing the app up

Two steps, both in `templates/webapp-conftest.py`:

**1. Build the frontend dist, rebuild only if stale.** Compare the newest source
mtime against `dist/index.html`; build with `bun run build` only when source is
newer (and `bun install --frozen-lockfile` only when `node_modules` is missing).
A clean run reuses the last build — don't pay a Vite build per session if nothing
changed.

**2. Launch the real app on a background thread** and wait for `/health` before
yielding its URL:

```python
@pytest.fixture(scope="session")
def backend_url(postgres_container, frontend_dist, _ensure_seed_schemas, env):
    pin_return_schema(env.schema)                 # harness lowercase-folding gotcha
    with run_app_in_thread(app="sampleapp.backend.app:app", port=find_free_port()) as url:
        yield url
```

`run_app_in_thread` runs uvicorn with `lifespan="on"` (so startup hooks fire),
polls `/health` until ready, and signals `server.should_exit = True` on teardown.
A free port per session lets modules run in parallel without collisions.

## The browser fixtures

Session-scoped browser (one launch), per-test context + page (isolation). The
auth header is injected at the **context** level so every page is authenticated:

```python
@pytest_asyncio.fixture(scope="session")
async def browser(playwright_instance):
    headless = os.environ.get("PLAYWRIGHT_HEADED") != "1"   # =1 to watch it drive
    b = await playwright_instance.chromium.launch(headless=headless)
    yield b
    await b.close()

@pytest_asyncio.fixture
async def context(browser, backend_url, tester_email):
    ctx = await browser.new_context(
        base_url=backend_url,
        extra_http_headers={"X-Auth-Email": tester_email},   # dev@nimbus.example in dev mode
    )
    yield ctx
    await ctx.close()

@pytest_asyncio.fixture
async def page(context):
    p = await context.new_page()
    yield p
    await p.close()
```

`asyncio_default_fixture_loop_scope = "session"` (set in `pyproject.toml` by
[postgres-test-harness]) is **required** — Playwright's async transports deadlock
if fixtures and tests don't share one event loop.

## A test seeds its own world, then drives the browser

Each test composes the data it needs through seeder fixtures (auto-truncated before
the test by the harness), loads the page, and asserts on what a **user perceives**:

```python
async def test_02_01_past_and_future_rows_render(
    page: Page,
    permissions_users: PermissionsUsersSeeder,
    orders_total: OrdersTotalSeeder,
) -> None:
    """A tab loaded with past + future weeks renders a grid with both."""
    await _seed_two_account_universe(permissions_users)
    for row in realistic_week_rows(n_past=2, n_future=2):
        await orders_total.add(**row)

    await page.goto("/orders")
    # a rendered currency cell proves rows loaded AND the formatter ran
    await expect(page.get_by_text("£100,000").first).to_be_visible()
```

## Assertion discipline — same as the frontend trophy

- **Query by accessibility first**: `get_by_role("heading", name="…")`,
  `get_by_alt_text("Acme")`, `get_by_role("button", name=/save/i)`. A `data-testid`
  is the last resort. This makes every E2E double as an a11y smoke check.
- **Assert what the user sees**, never app internals or network calls. Use
  `expect(...).to_be_visible()` / `to_have_text(...)`, not DOM-structure probing.
- **Wait, never sleep.** Playwright `expect` auto-retries; give first-render a
  generous timeout (`FIRST_RENDER_TIMEOUT_MS = 15_000`) because the app is booting
  a real backend, but never `asyncio.sleep`.
- **One journey per spec, numbered.** `test_<NN>_<MM>_<what>` (e.g.
  `test_02_01_…`) keeps related steps ordered and readable in the report. Keep the
  set thin — these are the money paths (view the grid, edit a plan, save), not every
  field.

## Headed / debugging

`PLAYWRIGHT_HEADED=1 uv run --no-sync pytest modules/orders/tests/webapp -k editing`
watches the browser drive the app. Use a `-k` filter — the full webapp suite boots
a container and an app, so run the slice you're debugging.

## moon / config

Webapp tests boot both the container and the app, so they're the slowest tier:
**smoke on PR, full suite nightly**. The task depends on `start-podman`; Playwright
**browsers** are not pip packages — install them as a task/CI step (`playwright
install --with-deps chromium`). `playwright`, `pytest-asyncio` are dev-dependencies.

## Templates

- `templates/webapp-conftest.py` — `build_frontend_dist`, `run_app_in_thread`, the
  `backend_url` fixture, and the browser/context/page fixtures.
- `templates/test_webapp.py` — a seeded, query-by-role E2E spec.
