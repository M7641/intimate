---
name: postgres-test-harness
description: >-
  Our test-database harness for the sampleapp apps — spin up a disposable Postgres
  container once per module test session, apply the Redshift compatibility layer,
  create the schemas, then seed tables through a generic `TableSeeder`
  (ensure_schema / truncate / add) with one seeder class per table, a seeder
  registry, an autouse truncate-between-tests fixture, and realistic-row data
  builders. This is the shared backing service that both the API tests and the
  Playwright webapp tests borrow — provision it first. Covers the three-layer
  production-safety guard (strip Redshift env, sentinel override, assert
  127.0.0.1), the session-scoped `AsyncDBActions` pool, and the
  `pin_return_schema` lowercase-folding gotcha. Use this whenever setting up the
  test database for a full-stack app, "provision a Postgres container for tests",
  "seed the test tables", "create the test database", "spin up testcontainers",
  "add a seeder for table X", "make a TableSeeder", "truncate between tests", or
  "build realistic seed data". Part of the e2e-testing set (the harness tier);
  the API tier is fastapi-api-testing, the browser tier is e2e-testing.
  For the per-language test kinds (unit/property/bench/mutation/fuzz) defer to
  python-testing-standards; for the warehouse table shapes under test defer to
  gateway-pattern and scd4-history.
---

# Postgres test harness

The shared backing service for application tests. One disposable Postgres container
per module test **session**, seeded through a uniform `TableSeeder`, torn down at
the end. Both higher tiers — [fastapi-api-testing] (API-only) and
[e2e-testing] (browser E2E) — borrow this exact harness. **Provision
it first; the other two are empty without it.**

> In the **sampleapp monorepo** this already exists as `common_py.testing.*` —
> reuse it (`postgres_session`, `async_pool`, `TableSeeder`, the
> `playwright_fixtures` plugin). The templates here are for bootstrapping a repo
> that has no `common_py`, and for understanding what each piece does.

## Why a real container, not SQLite

The production target is Redshift (a Postgres 8.0.2 fork); the dev/test target is a
real Postgres container. **Never fake it with SQLite** — server-side SQL, window
functions, `coalesce(edit, raw)` overlays and the SCD4 pivots all behave
differently. A test that passes on SQLite and fails on Redshift is worse than no
test. The container costs ~3s cold; a **session-scoped** fixture amortises that
across the whole module.

The container also loads a **Redshift compatibility layer** (`redshift_compat.sql`
— UDFs like `DATEDIFF`/`DATEADD`/`getdate()`) so the same SQL the app sends to
Redshift parses against the test Postgres. Apply it once, right after the container
starts, before any schema is created.

## The three-layer production-safety guard

The single most important property: **a test run must never reach production
Redshift.** Three independent layers enforce it, each a backstop for the last:

1. **Strip** `REDSHIFT_USERNAME` / `REDSHIFT_PASSWORD` / `REDSHIFT_HOST` from the
   env at import time — there is no real credential to find.
2. **Sentinel override** — pre-set `DB_CONNINFO_OVERRIDE` to a deliberately invalid
   value (`postgresql://__test_sentinel__@__container_not_started__:1/__no_db__`)
   so any pool built before the container is up fails *loud and fast*, not silently
   against a default host.
3. **Assert local** — once the container's real URL replaces the sentinel, assert
   the resolved conninfo contains `127.0.0.1` or `localhost` before yielding. A
   remote host here aborts the session.

Wire `enable_test_safety()` from the **root** `conftest.py` so it runs before any
module imports a pool. This guard is not optional ceremony — it is the reason
running the suite can never corrupt the warehouse.

## The session fixtures

```python
# modules/<module>/tests/conftest.py
@pytest.fixture(scope="session", autouse=True)
def postgres_container() -> Iterator[TestDatabase]:
    """Boot the container for this module's session. autouse so the
    sentinel→real-URL swap happens before any test runs."""
    with postgres_session() as handle:      # container + compat + schemas + safety
        yield handle

@pytest_asyncio.fixture(scope="session")
async def db_session(postgres_container) -> AsyncIterator[AsyncDBActions]:
    """One async pool per session — the container is one Postgres, so
    seed traffic and app traffic share it."""
    async with async_pool() as pool:
        yield pool
```

`postgres_session()` does container + compat + schema creation + the safety swap;
`async_pool()` opens an `AsyncDBActions` against it. Session scope for both — a
fresh container or pool per test would make a 5-second suite a 5-minute one.

## Seeding — one `TableSeeder` per table

A seeder is the test-side mirror of a [gateway-pattern] gateway: a bound
`(db, schema)` object that owns **one** table's DDL and exposes three atoms.

```python
class TableSeeder:
    """ensure_schema (idempotent CREATE) · truncate (between tests) · add (one row)."""

    def __init__(self, db: AsyncDBActions, schema: str, *, name: str, ddl: str) -> None:
        self.db, self.schema, self.name, self._ddl = db, schema, name, ddl

    @property
    def ddl(self) -> str:
        return Template(self._ddl).render(schema=self.schema, table=self.name)

    async def ensure_schema(self) -> None: ...   # CREATE SCHEMA + CREATE TABLE
    async def truncate(self) -> None: ...         # TRUNCATE TABLE
    async def add(self, **row) -> None:           # INSERT one row, columns named
        await self.db.insert_data(data=row, table_name=self.name, schema=self.schema)
```

A concrete seeder is tiny — a DDL template and a name:

```python
_DDL = "create table if not exists {{ schema }}.{{ table }} (...)"

class OrdersTotalSeeder(TableSeeder):
    def __init__(self, db, schema):
        super().__init__(db, schema, name="orders__total", ddl=_DDL)
```

Note `add(**row)` keeps the **column intent visible at the call site** —
`seeder.add(category_code="10", revenue=100_000.0)` reads as data, not as a
positional tuple you have to decode against the DDL.

### The registry + autouse truncate

List every seeder a module needs in one place, create all tables once per session,
and truncate them **before each test** so tests never clean up after themselves:

```python
RETURN_SCHEMA_SEEDERS = [PermissionsUsersSeeder, OrdersTotalSeeder, OrdersEditsSeeder,
                         UserSessionsSeeder]  # session middleware writes here — always seed it

@pytest_asyncio.fixture(scope="session")
async def _ensure_seed_schemas(db_session, env):
    for cls in RETURN_SCHEMA_SEEDERS:
        await cls(db_session, env.schema).ensure_schema()

@pytest_asyncio.fixture(autouse=True)
async def _truncate_seed_tables(db_session, _ensure_seed_schemas, env):
    for cls in RETURN_SCHEMA_SEEDERS:
        await cls(db_session, env.schema).truncate()
```

Create **all** tables once (even join targets a given test never writes to — an
empty table that must exist for a JOIN to resolve), truncate **all** before each.
This is what lets every test build its own small world from scratch and stay
isolated.

### Realistic-row builders

For tables with 40+ columns, a hand-written row per test is noise. Write a builder
that emits realistic rows and let the test override only what it asserts on:

```python
def realistic_week_rows(*, n_past=2, n_future=2, category_code="10") -> list[dict]:
    """Past weeks carry non-zero sales/stock (assert formatting); future
    weeks leave them 0 (assert recalculation populates them)."""
    ...
```

The builder encodes the *meaning* of the data (past vs future, the cascade); the
test reads as "given two past and two future weeks, …".

## The `pin_return_schema` gotcha

Postgres folds unquoted identifiers to **lowercase**; production Redshift schemas
are **uppercase**. If seeders write to `astral` and the app reads from `ASTRAL`,
every test silently returns zero rows. Pin the app's `return_schema` to the
lowercase test schema (`pin_return_schema(env.schema)`) when launching the app, so
seeder writes and app reads land in the same place. This bites at the seam between
this harness and the [e2e-testing] / [fastapi-api-testing] tiers —
fix it here, once.

## moon wiring

Container-backed test tasks depend on a `start-podman` task that auto-starts a
stopped Podman/Docker machine, so a developer with a cold daemon still gets a green
run. `testcontainers[postgres]`, `pytest-asyncio`, and `playwright` are
**dev-dependencies** in `pyproject.toml`, not proto tools. Pytest config:

```toml
[tool.pytest.ini_options]
asyncio_mode = "auto"
asyncio_default_fixture_loop_scope = "session"   # one loop — Playwright transports deadlock otherwise
testpaths = ["src", "modules/*/tests"]
env = ["TARGET_ENV=test", "TENANT=test", "LOG_LEVEL=WARNING"]
```

## Templates

- `templates/conftest.py` — the session container + pool fixtures, ready to adapt.
- `templates/seeder.py` — the `TableSeeder` base + a concrete seeder + its fixture
  + the registry/autouse-truncate wiring.

## Rollout

1. Add `enable_test_safety()` to the **root** `conftest.py` (the three-layer guard).
2. Drop the container + pool session fixtures into the module's `tests/conftest.py`.
3. Write one `TableSeeder` subclass per table the module reads, list them in the
   registry, add the `ensure_schema` + autouse `truncate` fixtures.
4. Hand off to [fastapi-api-testing] and [e2e-testing] — both consume
   `db_session` and the seeders you just built.
