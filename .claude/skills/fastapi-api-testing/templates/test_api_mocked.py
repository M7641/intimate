"""Mocked-DB API tests — fast routing / validation / shape / auth.

In the sampleapp monorepo, import ``mock_async_db`` from
``common_py.testing.mock_db``. This template carries it inline for a repo without
``common_py``.
"""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

import pytest
from fastapi.testclient import TestClient

from <module>.backend import app as app_module  # the module exposing `app`
from <module>.backend.auth import AuthUser, get_auth_user


def mock_async_db() -> AsyncMock:
    """A fully-wired AsyncDBActions double.

    - await db.load_data(...) -> [] by default (override per test)
    - await db.execute_query(...) -> None
    - async with db.connection() as conn: async with conn.cursor() as cur: ...
    """
    db = AsyncMock()  # use AsyncMock(spec=AsyncDBActions) when the type is importable
    db.load_data.return_value = []

    cursor = MagicMock(name="cursor")
    cursor.__aenter__ = AsyncMock(return_value=cursor)
    cursor.__aexit__ = AsyncMock(return_value=None)
    cursor.execute = AsyncMock(return_value=None)
    cursor.fetchone = AsyncMock(return_value=None)
    cursor.fetchall = AsyncMock(return_value=[])

    conn = MagicMock(name="conn")
    conn.cursor = MagicMock(return_value=cursor)
    conn.__aenter__ = AsyncMock(return_value=conn)
    conn.__aexit__ = AsyncMock(return_value=None)

    db.connection = MagicMock(return_value=conn)
    return db


@pytest.fixture()
def client(monkeypatch):
    """TestClient with the DB constructors patched to mocks and env set
    before the middleware stack is built."""
    monkeypatch.setenv("TARGET_ENV", "test")
    monkeypatch.setenv("TENANT", "test")
    monkeypatch.setattr(app_module, "AsyncDBActions", lambda **kw: mock_async_db())
    monkeypatch.setattr(app_module, "DuckDBActions", lambda **kw: AsyncMock())
    with TestClient(app_module.app, raise_server_exceptions=False) as c:
        yield c


@pytest.fixture()
def authed_client(client):
    client.headers.update({"X-Auth-Email": "dev@nimbus.example"})
    return client


@pytest.fixture()
def override_auth():
    app_module.app.dependency_overrides[get_auth_user] = lambda: AuthUser(email="dev@nimbus.example")
    yield
    app_module.app.dependency_overrides.pop(get_auth_user, None)


class TestHealth:
    def test_liveness_probe_returns_ok(self, client: TestClient):
        resp = client.get("/health")
        assert resp.status_code == 200
        assert resp.json() == {"status": "ok"}

    def test_unknown_api_path_returns_404(self, client: TestClient):
        assert client.get("/api/does-not-exist").status_code == 404


class TestAuth:
    def test_request_without_auth_header_is_rejected(self, client: TestClient):
        assert client.get("/api/orders/total").status_code in (401, 403)
