---
name: gateway-pattern
description: >-
  Our gateway pattern for database access in the sampleapp apps — wrap the raw
  connection layer (`DBActionsProtocol`: load_data / execute_query / insert_data /
  connection) into a small bound class that manages ONE table artifact and exposes
  intent-revealing methods, instead of scattering SQL through route handlers. Covers
  the three table roles (`upstream` read-only, `owned` SCD4 live+history, `event_log`
  append-only) and the method shape each implies, the `<name>_gateway/` folder layout
  (artifact.py + gateway.py + sql/ + __init__.py), the `(db, schema)` constructor
  convention, composing several gateways into one atomic transaction via
  `apply_edit_with_cursor`, and — the other half — what belongs in the FastAPI
  `lifespan` (the DB pool, DuckDB, SqlResultCache, caches) versus what gets
  instantiated per request (the gateways themselves), and how `request.state` +
  `Annotated[T, Depends(getter)]` wire the two together. Use this whenever adding or
  reviewing a data-access class in these apps, creating a new `*_gateway`, deciding
  "where does the db connection / cache / client live", wiring an object into the app
  lifespan, refactoring SQL out of a route, or asked "use the gateway pattern" /
  "where should this connector go". For the SCD4 history-table SQL itself defer to
  scd4-history; for broader FastAPI structure defer to fastapi-python.
---

# The gateway pattern

A **gateway** takes the raw connection layer and organises it into a class that
manages **one abstract data object** — a table, or a table + its history sibling.
It is the single seam between domain code and SQL: route handlers call
`gateway.record_many(...)` or `gateway.apply_edit(...)`, never `db.insert_data` or a
raw `SELECT`.

```
DBActionsProtocol   (raw connection layer: load_data / execute_query / insert_data / connection)
        │  bound by
        ▼
   <X>Gateway       (db + schema + ONE TableArtifact → intent-revealing methods)
        │  manages
        ▼
   TableArtifact    (the abstract object: name, role, columns, DDL)
```

The gateway **borrows** the connection; it does not own it. That is the whole
reason it can be instantiated cheaply per request while the *connection* lives once
in the app lifespan (see [Lifespan](#what-lives-in-the-lifespan) below).

## Three roles, one shape

The table's `role` (declared on the `TableArtifact`) decides what the gateway is
*allowed* to do, and therefore which methods it exposes:

| `role` | meaning | gateway exposes | example |
|---|---|---|---|
| `upstream` | read-only for every app | `load_data`-backed readers (`list_for_user`, `get_one`) | `DepartmentMenuGateway` |
| `owned` | read+write by exactly ONE app | readers **+** SCD4 `apply_edit` (live+history, atomic) | `CommentsGateway` |
| `event_log` | owned + append-only | `record_many` (insert only) | `OrdersEditsGateway` |

Name the role first. It is the contract — a reviewer reading `role="event_log"`
knows there is no `UPDATE` path to look for. Don't add an `update` method to an
`event_log` gateway; if you need one, the role is wrong.

## Folder layout

One folder per gateway, named `<thing>_gateway/`, co-locating the three concerns:

```
<thing>_gateway/
  __init__.py     # re-export the artifact(s) + the gateway class
  artifact.py     # the TableArtifact(s) — the abstract object
  gateway.py      # the Gateway class — the bound accessor
  sql/            # .sql files, one statement each (SCD4 / complex reads only)
    select_one_live.sql
    delete_live.sql
    insert_live.sql
    insert_history.sql
```

A simple read-only gateway needs no `sql/` dir — hold the one query as a
module-level template string in `gateway.py` (see `DepartmentMenuGateway`). Reach
for `sql/` files once a gateway runs several statements or the SQL is long enough to
drown the Python.

## The constructor convention

Every gateway has the **same** constructor: a connection abstraction and a schema.

```python
class OrdersEditsGateway:
    """Append-only write path for ORDERS_EDITS."""

    def __init__(self, db: DBActionsProtocol, schema: str) -> None:
        self._db = db
        self._schema = schema
        self._artifact = ORDERS_EDITS
```

Three rules that make the pattern hold together:

1. **Type the parameter as `DBActionsProtocol`, not `AsyncDBActions`.** The Protocol
   is satisfied by both the real Redshift pool *and* the in-process DuckDB mock, so
   the identical gateway runs in production and in tests with no swap. Only type it
   as the concrete `AsyncDBActions` when the method genuinely needs `connection()`
   for a multi-statement transaction (SCD4 writers).
2. **Carry no expensive state.** `(db, schema)` and an artifact reference — that's
   it. This is what licenses instantiate-per-request: `OrdersEditsGateway(db, env.return_schema).record_many(rows)`.
   No pooling, no caching of the gateway itself.
3. **Bind the artifact in the constructor**, so the table name is reached through
   `self._artifact.name`, never a string literal in a method.

## Atomic composition across gateways

An `owned` (SCD4) gateway exposes its write **twice**:

- `apply_edit(*, ..., audit)` — opens its own connection. Use when the route mutates
  one table.
- `apply_edit_with_cursor(cur, *, ..., audit)` — runs on a cursor the **caller**
  owns. Use when a route must mutate several tables in one transaction.

```python
# Route saving forecast edits AND comments together — one transaction, both gateways:
async with db.connection() as conn:
    async with conn.cursor(row_factory=dict_row) as cur:
        for edit in body.edits:
            if edit.column_type == "forecast":
                await forecast_gateway.apply_edit_with_cursor(cur, ...)
            elif edit.column_type == "comments":
                await comments_gateway.apply_edit_with_cursor(cur, ...)
```

The `apply_edit` form is just `apply_edit_with_cursor` wrapped in
`async with self._db.connection()`. Write the cursor form, derive the other from it
— don't duplicate the choreography.

> The SCD4 live+history choreography (delete-live → insert-live → insert-history,
> the `row_hash` / `previous_row_hash` chain, `valid_from`/`valid_to`) is the
> **scd4-history** skill's territory. This skill is about the *class* that wraps it.

## What lives in the lifespan

The other half of the question: a gateway is cheap and per-request, but the things
it *borrows* are expensive and long-lived. Those go in the FastAPI `lifespan`,
created once at startup and torn down at shutdown.

**The dividing line:** put it in the lifespan if creating it is expensive (opens
sockets, allocates a pool, builds an engine) **or** it holds shared mutable state
the whole app reads. Instantiate it per request if it's a cheap bound view over
something already in the lifespan — that's exactly a gateway.

| Lives in lifespan (`request.state`) | Instantiated per request |
|---|---|
| `db` — the `AsyncDBActions` pool (Redshift / Postgres testcontainer) | every `*Gateway` |
| `duck` — in-process `DuckDBActions` result engine | request-scoped shaping helpers |
| `sql_cache` — `SqlResultCache` over `duck` | |
| `pending_edits`, `mem_cache`, `env_manager`, `s3` | |

```python
class State(TypedDict):
    db: AsyncDBActions          # the warehouse — owns every persisted table
    duck: DuckDBActions         # separate in-memory engine — the result cache
    sql_cache: SqlResultCache
    mem_cache: InMemoryCache
    env_manager: EnvManager


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[State]:
    db = AsyncDBActions(min_size=0, max_size=5)
    await db.open()                       # open the pool ONCE
    duck = DuckDBActions(path=":memory:")
    await duck.open()
    sql_cache = SqlResultCache(duck=duck)
    await sql_cache.ensure_schema()
    try:
        yield State(db=db, duck=duck, sql_cache=sql_cache, ...)
    finally:
        await duck.close()                # tear down in reverse
        await db.close()
```

Two engines, two jobs: **`db` is the warehouse** (the source of truth, every table
a gateway touches); **`duck` is the result cache** (always in-memory, even when
`db` is also DuckDB in tests). Keeping them as distinct entries on `State` stops the
two from being confused — a gateway writes to `db`; the route invalidates
`sql_cache` after the write.

### Wiring lifespan objects into routes

Resolve from `request.state` through a getter, then alias with `Annotated` so route
signatures stay clean:

```python
# common_py/api/db.py
def get_db(request: Request) -> AsyncDBActions:
    return request.state.db

Db = Annotated[AsyncDBActions, Depends(get_db)]          # reusable alias
SqlCache = Annotated[SqlResultCache, Depends(get_sql_cache)]
```

```python
# a route — db comes from the lifespan; the gateway is built right here
@router.post("/save")
async def save_orders_edits(body: OrdersSaveRequest, db: Db, sql_cache: SqlCache, env: Env, user: CurrentUser):
    rows = _expand_edits(body, edited_by=user.email)
    await OrdersEditsGateway(db, env.return_schema).record_many(rows)   # per-request gateway
    await sql_cache.invalidate_all()                                  # lifespan-owned cache
```

A getter that reaches a missing `request.state.X` raises `AttributeError` at request
time — intentional. A misconfigured app (route mounted without the matching
lifespan entry) fails loudly rather than silently degrading.

## When NOT to reach for this

- A genuine one-off query used in exactly one place with no role/identity concerns —
  a module-level template + a single `db.load_data` call is fine; a class adds
  ceremony. (But the moment a second caller appears, or it touches an `owned` table,
  promote it to a gateway.)
- Cross-table read joins that don't *own* any table — those are query functions, not
  gateways. A gateway manages one artifact; a reporting join manages none.

## Doing the pattern well — review checklist

When writing or reviewing a gateway, hold it to these:

- **One artifact per gateway.** Manages one table (or one table+history pair).
  Touching three unrelated tables → it's a service, not a gateway; split it.
- **Role matches methods.** No `update`/`delete` on an `event_log`; no silent
  overwrite on an `owned` table that skips the history insert.
- **`DBActionsProtocol` in the signature** unless `connection()` is actually needed
  — keeps the test mock drop-in.
- **No table-name string literals** in methods — go through `self._artifact.name`.
- **SQL in `sql/` or a module constant**, never f-string-interpolated with request
  data. Use the `params` (server-bound) / `static_params` (Jinja schema/table)
  split.
- **Naming is uniform.** Call it `<Thing>Gateway` and the folder `<thing>_gateway`.
  (The codebase has a stray `forecast_edits_repository` — "repository" vs "gateway"
  for the same shape is noise. Prefer `gateway`.)
- **DRY the SQL-loading boilerplate.** The
  `coerce(apply_static((_SQL_DIR / "x.sql").read_text(), self._static))` triple
  repeats per statement — a one-line `self._sql("x")` helper on the class reads far
  better. See `references/templates.md`.
- **Per-request, no cached state.** If you feel tempted to cache the gateway
  instance, the thing you actually want to cache belongs in the lifespan instead.

## Scaffolds

Copy-paste starting points for each of the three roles, the `__init__.py`
re-export, and the `self._sql` helper are in **`references/templates.md`**.
