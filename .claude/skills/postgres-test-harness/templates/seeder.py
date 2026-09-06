"""TableSeeder base + a concrete seeder + its fixtures + the registry wiring.

In the sampleapp monorepo, import ``TableSeeder`` from ``common_py.testing.seeder``
and only write the concrete subclasses + the registry. This template carries the
base too, for a repo without ``common_py``.
"""

from __future__ import annotations

from collections.abc import AsyncIterator
from typing import Any

import pytest_asyncio
from jinja2 import Template
from psycopg import sql

from common_py.io.db import AsyncDBActions


class TableSeeder:
    """Generic per-table seeder, composed from three atoms.

    - ensure_schema: idempotent CREATE SCHEMA + CREATE TABLE
    - truncate:      clear the table between tests
    - add:           insert one row, columns named at the call site
    """

    def __init__(self, db: AsyncDBActions, schema: str, *, name: str, ddl: str) -> None:
        self.db = db
        self.schema = schema
        self.name = name
        self._ddl_template = ddl

    @property
    def fully_qualified_table_name(self) -> str:
        return f"{self.schema}.{self.name}"

    @property
    def ddl(self) -> str:
        return Template(self._ddl_template).render(schema=self.schema, table=self.name)

    async def ensure_schema(self) -> None:
        async with self.db.connection() as conn, conn.cursor() as cur:
            await cur.execute(
                sql.SQL("create schema if not exists {}").format(
                    sql.Identifier(self.schema)
                )
            )
            await cur.execute(self.ddl.encode())

    async def truncate(self) -> None:
        async with self.db.connection() as conn, conn.cursor() as cur:
            await cur.execute(
                sql.SQL("truncate table {}.{}").format(
                    sql.Identifier(self.schema), sql.Identifier(self.name)
                )
            )

    async def add(self, **row: Any) -> None:
        await self.db.insert_data(data=row, table_name=self.name, schema=self.schema)


# --- a concrete seeder: one DDL template + a name --------------------------
_ORDERS_TOTAL_DDL = """
create table if not exists {{ schema }}.{{ table }} (
    date_week           date,
    category_code     varchar(16),
    sub_category_code   varchar(16),
    product_type_group  varchar(16),
    revenue        double precision,
    revenue_ly     double precision
    -- ... the rest of the production columns
)
"""


class OrdersTotalSeeder(TableSeeder):
    """orders__total — source rows for the total-scope endpoint."""

    def __init__(self, db: AsyncDBActions, schema: str) -> None:
        super().__init__(db, schema, name="orders__total", ddl=_ORDERS_TOTAL_DDL)


# --- the registry: every table the module needs, in one place --------------
RETURN_SCHEMA_SEEDERS: list[type[TableSeeder]] = [
    OrdersTotalSeeder,
    # PermissionsUsersSeeder,
    # OrdersEditsSeeder,
    # UserSessionsSeeder,  # the session middleware writes here — always include it
]


# --- create all tables once, truncate all before each test -----------------
@pytest_asyncio.fixture(scope="session")
async def _ensure_seed_schemas(db_session: AsyncDBActions, env) -> None:
    for cls in RETURN_SCHEMA_SEEDERS:
        await cls(db_session, env.schema).ensure_schema()


@pytest_asyncio.fixture(autouse=True)
async def _truncate_seed_tables(
    db_session: AsyncDBActions, _ensure_seed_schemas, env
) -> None:
    for cls in RETURN_SCHEMA_SEEDERS:
        await cls(db_session, env.schema).truncate()


# --- one fixture per seeder, so a test depends only on the tables it touches
@pytest_asyncio.fixture
async def orders_total(db_session: AsyncDBActions, env) -> AsyncIterator[OrdersTotalSeeder]:
    yield OrdersTotalSeeder(db_session, env.schema)
