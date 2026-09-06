"""Session-scoped Postgres test harness — adapt per module.

In the sampleapp monorepo, import these from ``common_py.testing`` instead of
copying. This template is for a repo without ``common_py``: it shows the whole
chain (container -> compat -> schemas -> safety -> pool) self-contained.

Place the production-safety guard in the ROOT conftest.py so it runs before any
module imports a connection pool:

    # <repo-root>/conftest.py
    from common_py.testing.fixtures import enable_test_safety
    enable_test_safety()
    pytest_plugins = ["common_py.testing.playwright_fixtures"]
"""

from __future__ import annotations

import os
from collections.abc import AsyncIterator, Iterator
from contextlib import asynccontextmanager, contextmanager
from dataclasses import dataclass

import pytest
import pytest_asyncio

# Replace with your own pool type / conninfo resolver.
from common_py.io.db import AsyncDBActions
from common_py.io.db import redshift_conninfo

DEFAULT_SCHEMAS: tuple[str, ...] = ("astral", "stage")
_SENTINEL = "postgresql://__test_sentinel__@__container_not_started__:1/__no_db__"


@dataclass
class TestDatabase:
    """Handle to the running test container."""

    conninfo: str


# --- Layer 1+2 of the safety guard: run from the ROOT conftest, at import time ---
def enable_test_safety() -> None:
    for var in ("REDSHIFT_USERNAME", "REDSHIFT_PASSWORD", "REDSHIFT_HOST"):
        os.environ.pop(var, None)
    os.environ["DB_CONNINFO_OVERRIDE"] = _SENTINEL


@contextmanager
def postgres_test_container(
    image: str = "postgres:18-alpine",
    schemas: tuple[str, ...] = DEFAULT_SCHEMAS,
) -> Iterator[TestDatabase]:
    """Start Postgres, apply the Redshift compat layer, create schemas, yield."""
    from testcontainers.postgres import PostgresContainer

    container = PostgresContainer(image, driver=None)
    container.start()
    try:
        handle = TestDatabase(conninfo=container.get_connection_url())
        # _apply_compat(handle)   # load redshift_compat.sql (DATEDIFF/DATEADD/getdate UDFs)
        # _create_schemas(handle, schemas)
        yield handle
    finally:
        container.stop()


@contextmanager
def postgres_session() -> Iterator[TestDatabase]:
    """Layer 3 of the guard: swap the sentinel for the real URL, assert local."""
    previous = os.environ.get("DB_CONNINFO_OVERRIDE")
    with postgres_test_container() as handle:
        os.environ["DB_CONNINFO_OVERRIDE"] = handle.conninfo
        resolved = redshift_conninfo()
        assert "127.0.0.1" in resolved or "localhost" in resolved, (
            f"test pool resolved to a non-local host: {resolved!r}"
        )
        try:
            yield handle
        finally:
            if previous is None:
                os.environ.pop("DB_CONNINFO_OVERRIDE", None)
            else:
                os.environ["DB_CONNINFO_OVERRIDE"] = previous


@asynccontextmanager
async def async_pool(
    min_size: int = 1, max_size: int = 2
) -> AsyncIterator[AsyncDBActions]:
    pool = AsyncDBActions(min_size=min_size, max_size=max_size)
    await pool.open(wait=True)
    try:
        yield pool
    finally:
        await pool.close()


# --- the two fixtures every module's tests/conftest.py exposes ---------------
@pytest.fixture(scope="session", autouse=True)
def postgres_container() -> Iterator[TestDatabase]:
    """autouse: the sentinel->real-URL swap must happen before any test runs."""
    with postgres_session() as handle:
        yield handle


@pytest_asyncio.fixture(scope="session")
async def db_session(postgres_container: TestDatabase) -> AsyncIterator[AsyncDBActions]:
    async with async_pool() as pool:
        yield pool
