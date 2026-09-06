"""Seeded-container API tests — real server-side SQL, no browser.

Depends on the postgres-test-harness fixtures (`db_session`, `env`) and the
seeders (`orders_total`, `orders_edits`). The app is bound to the TEST pool via
ASGITransport so requests never open a Redshift connection.
"""

from __future__ import annotations

import datetime
from collections.abc import AsyncIterator

import pytest
import pytest_asyncio
from httpx import ASGITransport, AsyncClient

from <module>.backend.app import build_app          # a factory that accepts db=...
from common_py.testing.fixtures import pin_return_schema

MONDAY = datetime.date(2026, 6, 22)


@pytest_asyncio.fixture
async def api(db_session, env) -> AsyncIterator[AsyncClient]:
    """In-process client whose app reads from the test container pool."""
    pin_return_schema(env.schema)  # match seeder writes (lowercase) to app reads
    transport = ASGITransport(app=build_app(db=db_session))
    async with AsyncClient(
        transport=transport,
        base_url="http://test",
        headers={"X-Auth-Email": "dev@nimbus.example"},
    ) as client:
        yield client


async def test_total_endpoint_overlays_edits_on_raw(api, orders_total, orders_edits):
    """The /total endpoint must return edit values overlaid on raw rows —
    behaviour that only the real coalesce(edit, raw) SQL proves."""
    await orders_total.add(category_code="10", revenue=100_000.0, date_week=MONDAY)
    await orders_edits.add(
        category_code="10",
        column_name="revenue",
        edit_value=120_000.0,
        date_week=MONDAY,
    )

    resp = await api.get("/api/orders/total?week=2026-06-22")

    assert resp.status_code == 200
    assert resp.json()["rows"][0]["revenue"] == 120_000.0


async def test_missing_required_query_param_is_422(api):
    assert (await api.get("/api/orders/total")).status_code == 422
