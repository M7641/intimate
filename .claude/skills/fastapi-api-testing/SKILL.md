---
name: fastapi-api-testing
description: >-
  How we run API-only tests for a FastAPI app in the sampleapp stack — exercise the
  HTTP contract without a browser, in two flavours: fast mocked-DB tests (TestClient
  + AsyncMock, dependency_overrides, injected auth headers) for routing / validation
  / shape, and seeded-container integration tests (httpx.AsyncClient over
  ASGITransport, or the running app, against the postgres-test-harness) for anything
  whose behaviour depends on real server-side SQL. Covers the AsyncDBActions test
  double, overriding the lifespan / get_db / get_auth_user, the X-Auth-Email header
  convention, what to assert (status + JSON contract, not internals), and the
  tests/api/ folder layout. Use this whenever testing FastAPI endpoints, "test my
  API", "test this route / handler", "API integration test", "test the endpoint
  against a real database", "mock the DB for a route test", "override a FastAPI
  dependency in tests", "test auth on an endpoint", or "test the JSON my API
  returns". Part of the e2e-testing set (the API tier); the database it runs
  against is postgres-test-harness, the browser tier is e2e-testing.
  For the per-language test kinds (property/bench/mutation/fuzz) defer to
  python-testing-standards; the data-access classes under test are gateway-pattern.
---

# FastAPI API testing

Test the **API's HTTP contract** — status codes, JSON shape, validation, auth —
without a browser. Two flavours, chosen by what the endpoint actually depends on:

| Flavour | Backing DB | Use for | Speed |
|---|---|---|---|
| **Mocked-DB** | `AsyncMock` double | routing, validation, auth, response shape, error mapping | ~5ms |
| **Seeded-container** | [postgres-test-harness] | anything whose answer depends on real server-side SQL (overlays, pivots, SCD4 reads) | ~tens of ms |

The rule mirrors the language standards: **don't fake Postgres if the behaviour
*is* the SQL.** A route that just validates a body and inserts → mocked-DB is
honest and fast. A route that returns the result of a `coalesce(edit, raw)` overlay
or an SCD4 pivot → it must run against the real seeded container, or the test
proves nothing.

## Folder layout

```
modules/<module>/tests/
├── conftest.py        # the postgres-test-harness session fixtures
└── api/               # API-only tests — no browser
    ├── conftest.py    # client fixtures (mocked + seeded), auth helpers
    └── test_*.py
```

> The existing repo modules name this folder `backend/` and keep mostly mocked-DB
> tests there. `api/` is the clearer name for the tier; treat the two as the same
> role. The browser tier is always `webapp/` ([e2e-testing]).

## Flavour 1 — mocked-DB (fast, the default for shape & routing)

Patch the app's `AsyncDBActions` / `DuckDBActions` constructors to return mocks
*before* `TestClient` builds the lifespan, set the env the middleware reads, and
override the auth dependency. The DB double is fully wired — `load_data` returns
`[]`, `connection()` / `cursor()` work as async context managers — so handler code
runs unchanged.

```python
@pytest.fixture()
def client(monkeypatch):
    monkeypatch.setenv("TARGET_ENV", "test")
    monkeypatch.setenv("TENANT", "test")
    monkeypatch.setattr(app_module, "AsyncDBActions", lambda **kw: mock_async_db())
    monkeypatch.setattr(app_module, "DuckDBActions", lambda **kw: AsyncMock())
    with TestClient(app_module.app, raise_server_exceptions=False) as c:
        yield c

@pytest.fixture()
def authed_client(client):
    client.headers.update({"X-Auth-Email": "test@example.com"})
    return client
```

Two reasons it's `monkeypatch.setattr` on the *constructor*, not a
`dependency_overrides` of `get_db`: the lifespan builds the pool itself (it isn't a
dependency), and the env must be set before the middleware stack is constructed.
For dependencies that *are* injected (auth), use `dependency_overrides`:

```python
@pytest.fixture()
def override_auth():
    app_module.app.dependency_overrides[get_auth_user] = lambda: AuthUser(email="dev@nimbus.example")
    yield
    app_module.app.dependency_overrides.pop(get_auth_user, None)
```

The DB double (`mock_async_db()` — see `templates/test_api_mocked.py`) is a
`AsyncMock(spec=AsyncDBActions)` with the cursor/connection async-context chain
pre-wired. Set return values per test when a handler reads:
`client.app.state...` or `db.load_data.return_value = [{"x": 1}]`.

```python
class TestHealth:
    def test_liveness_probe_returns_ok(self, client: TestClient):
        resp = client.get("/health")
        assert resp.status_code == 200
        assert resp.json() == {"status": "ok"}

    def test_unknown_api_path_returns_404(self, client: TestClient):
        assert client.get("/api/does-not-exist").status_code == 404
```

## Flavour 2 — seeded-container (real SQL)

When the endpoint's answer *is* a query, run it against [postgres-test-harness].
Build the world with seeders, hit the endpoint with `httpx.AsyncClient` over
`ASGITransport` (in-process, no socket), and assert on the JSON contract.

```python
@pytest_asyncio.fixture
async def api(db_session, env) -> AsyncIterator[AsyncClient]:
    pin_return_schema(env.schema)                 # lowercase-folding gotcha (harness)
    transport = ASGITransport(app=build_app(db=db_session))   # app bound to the test pool
    async with AsyncClient(transport=transport, base_url="http://test",
                           headers={"X-Auth-Email": "dev@nimbus.example"}) as c:
        yield c

async def test_total_endpoint_overlays_edits_on_raw(api, orders_total, orders_edits):
    await orders_total.add(category_code="10", revenue=100_000.0, date_week=MONDAY)
    await orders_edits.add(category_code="10", column_name="revenue",
                         edit_value=120_000.0, date_week=MONDAY)

    resp = await api.get("/api/orders/total?week=2026-06-22")

    assert resp.status_code == 200
    # the edit overlays the raw value — this is the behaviour only real SQL proves
    assert resp.json()["rows"][0]["revenue"] == 120_000.0
```

The key move is binding the app to the **test pool** (`db_session`) instead of
letting its lifespan open a Redshift pool — pass it into `build_app(db=...)`, or
override `get_db` to return `db_session`. The [postgres-test-harness] safety guard
guarantees that pool is the local container.

## What to assert — and what not to

- **Assert the contract**: status code, the JSON shape and values a consumer relies
  on, the error body on a 4xx, the `Location`/headers that matter.
- **Don't assert internals**: never reach into `app.state`, never assert a specific
  SQL string was sent, never check that a gateway method was called *N* times as a
  proxy for behaviour. Those tests pass while the API is broken and break when you
  refactor working code — the same anti-pattern the frontend standard names for the
  UI.
- **Cover the unhappy paths**: a missing required field → 422, an unauthorised
  caller → 401/403, a not-found id → 404. Validation and auth are part of the
  contract, not an afterthought.

## Auth in tests

Every hub app reads the caller from the `X-Auth-Email` header (dev mode accepts
`dev@nimbus.example`). Inject it on the client (mocked flavour) or via
`extra_http_headers` / `dependency_overrides[get_auth_user]` (seeded flavour).
Always include at least one test that a request **without** it is rejected — auth
that's never tested is auth that silently regresses.

## moon / config

API tests run under the inherited `test` task (`uv run --no-sync pytest`). The
seeded flavour pulls in the container, so the task depends on `start-podman` (see
[postgres-test-harness]). `httpx`, `pytest-asyncio` are dev-dependencies.

## Templates

- `templates/test_api_mocked.py` — the `mock_async_db()` double + client fixtures +
  a couple of mocked-DB tests.
- `templates/test_api_seeded.py` — the `httpx.AsyncClient` + ASGITransport fixture
  bound to the harness pool + a real-SQL test.
