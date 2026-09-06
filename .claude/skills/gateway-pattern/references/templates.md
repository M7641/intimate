# Gateway scaffolds

Copy-paste starting points for the three roles. Replace `Thing` / `thing` / table
names. Each gateway folder is `<thing>_gateway/` with `__init__.py`, `artifact.py`,
`gateway.py`, and (for SCD4) a `sql/` dir.

---

## `__init__.py` — re-export the public surface

```python
"""``thing__table_v2`` — one-line description of the abstract object."""

from <pkg>.thing_gateway.artifact import THING
from <pkg>.thing_gateway.gateway import ThingGateway

__all__ = ["THING", "ThingGateway"]
```

---

## `artifact.py` — declare the abstract object

```python
"""``thing__table_v2`` — what it holds, who reads it, who writes it."""

from common_py.db_artifacts import ColumnSpec, TableArtifact

THING = TableArtifact(
    name="thing__table_v2",
    role="event_log",          # "upstream" | "owned" | "event_log"
    owner="thing-app",
    columns={
        "business_key": ColumnSpec(pg_type="varchar(128)", nullable=False),
        "value":        ColumnSpec(pg_type="double precision"),
        "edit_time":    ColumnSpec(pg_type="timestamp", nullable=False),
        "edited_by":    ColumnSpec(pg_type="varchar(256)", nullable=False),
    },
    # Omit `ddl=` to let render_ddl build it from `columns`. Provide an explicit
    # Jinja `ddl="""..."""` only when you need Redshift-specific types (e.g.
    # VARCHAR(65535) for VARCHAR(MAX)) written so Postgres/DuckDB also parse them.
)
```

For an `owned` SCD4 table, declare **two** artifacts (`THING` + `THING_HISTORY`) and
also set `row_key_columns` / `business_id_columns`. The history table's columns +
SQL are the **scd4-history** skill's job.

---

## Role 1 — `upstream` (read-only)

No `sql/` dir; the query is a module constant.

```python
"""Read access to ``thing__table_v2``."""

from common_py.io.db import DBActionsProtocol

# Only {{ schema }} is templated; table names are platform constants.
# %(name)s is psycopg's bound-param style; %% is a literal % .
_SELECT = """
select business_key, value
from {{ schema }}.thing__table_v2
where edited_by = %(user_email)s
order by 1
"""


class ThingGateway:
    """Bound (db, schema) accessor for thing reads."""

    def __init__(self, db: DBActionsProtocol, schema: str) -> None:
        self._db = db
        self._schema = schema

    async def list_for_user(self, user_email: str) -> list[dict]:
        return await self._db.load_data(
            _SELECT,
            params={"user_email": user_email},
            static_params={"schema": self._schema},
        )
```

---

## Role 2 — `event_log` (append-only)

```python
"""Append-only write path for THING."""

from typing import Any

from common_py.io.db import DBActionsProtocol
from <pkg>.thing_gateway.artifact import THING


class ThingGateway:
    """Append-only writer for THING."""

    def __init__(self, db: DBActionsProtocol, schema: str) -> None:
        self._db = db
        self._schema = schema
        self._artifact = THING

    async def record_many(self, rows: list[dict[str, Any]]) -> None:
        """Append one row per edit. Append-only: live state is the latest
        row per natural key by edit_time."""
        if not rows:
            return
        await self._db.insert_data(
            data=rows,
            schema=self._schema,
            table_name=self._artifact.name,
        )
```

---

## Role 3 — `owned` (SCD4 live + history, atomic)

Note the concrete `AsyncDBActions` type here — this role needs `connection()` for a
multi-statement transaction, which the Protocol's read helpers don't cover. Expose
the write **twice**: `apply_edit` (own connection) and `apply_edit_with_cursor`
(caller's cursor, for composing several gateways in one transaction). The `_sql`
helper kills the per-statement boilerplate.

```python
"""Transactional gateway for thing__table_v2 + history (SCD4)."""

import datetime
from pathlib import Path
from typing import Literal

from psycopg.rows import dict_row

from common_py.io.db import AsyncDBActions
from common_py.io.db.utils import apply_static, coerce
from <pkg>.audit import AuditContext
from <pkg>.thing_gateway.artifact import THING, THING_HISTORY

_SQL_DIR = Path(__file__).parent / "sql"
ChangeType = Literal["insert", "update", "delete"]


class ThingGateway:
    """Owns the live + history invariant for thing."""

    def __init__(self, db: AsyncDBActions, schema: str) -> None:
        self._db = db
        self._schema = schema
        self._static = {
            "schema": schema,
            "table": THING.name,
            "history_table": THING_HISTORY.name,
        }

    def _sql(self, name: str) -> str:
        """Load sql/<name>.sql with static (schema/table) params applied."""
        text = (_SQL_DIR / f"{name}.sql").read_text()
        return coerce(apply_static(text, self._static))

    async def apply_edit(self, *, business_key: str, value: float | None,
                         audit: AuditContext) -> ChangeType:
        async with self._db.connection() as conn:
            async with conn.cursor(row_factory=dict_row) as cur:
                return await self.apply_edit_with_cursor(
                    cur, business_key=business_key, value=value, audit=audit
                )

    async def apply_edit_with_cursor(self, cur, *, business_key: str,
                                     value: float | None,
                                     audit: AuditContext) -> ChangeType:
        now = datetime.datetime.now(datetime.timezone.utc)
        prior = await self._fetch_live(cur, business_key)
        change_type: ChangeType = "update" if prior else "insert"

        await cur.execute(self._sql("delete_live"), {"row_key": business_key})
        await cur.execute(self._sql("insert_live"),
                          {"row_key": business_key, "value": value, "edit_time": now})
        await cur.execute(self._sql("insert_history"), {
            "row_key": business_key, "value": value,
            "valid_from": now, "valid_to": None, "change_type": change_type,
            "changed_by": audit.changed_by, "request_id": audit.request_id,
            # ... + row_hash / previous_row_hash — see scd4-history
        })
        return change_type

    async def _fetch_live(self, cur, row_key: str) -> dict | None:
        await cur.execute(self._sql("select_one_live"), {"row_key": row_key})
        return await cur.fetchone()
```

---

## Lifespan entry + DI wiring

```python
# app.py
class State(TypedDict):
    db: AsyncDBActions
    env_manager: EnvManager

@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[State]:
    db = AsyncDBActions(min_size=0, max_size=5)
    await db.open()
    try:
        yield State(db=db, env_manager=EnvManager())
    finally:
        await db.close()

# common_py/api/db.py — getter + alias
def get_db(request: Request) -> AsyncDBActions:
    return request.state.db

Db = Annotated[AsyncDBActions, Depends(get_db)]

# routes.py — db from lifespan, gateway per request
@router.post("/save")
async def save(body: SaveRequest, db: Db, env: Env, user: CurrentUser):
    await ThingGateway(db, env.return_schema).record_many(rows)
```
