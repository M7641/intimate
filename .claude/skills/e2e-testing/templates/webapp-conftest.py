"""Webapp E2E fixtures — build dist, run the app, drive a browser.

In the sampleapp monorepo, import build_frontend_dist / run_app_in_thread from
``common_py.testing.webapp`` and register the shared browser fixtures via
``pytest_plugins = ["common_py.testing.playwright_fixtures"]`` in the root
conftest. This template carries them inline for a repo without ``common_py``.
"""

from __future__ import annotations

import os
import socket
import subprocess
import threading
import time
from collections.abc import AsyncIterator, Iterator
from contextlib import contextmanager
from pathlib import Path

import httpx
import pytest
import pytest_asyncio
import uvicorn
from playwright.async_api import (
    Browser,
    BrowserContext,
    Page,
    Playwright,
    async_playwright,
)


# --- bring the app up -------------------------------------------------------
def find_free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def build_frontend_dist(frontend_dir: Path) -> Path:
    """Build the Vite bundle, rebuilding only if source is newer than dist."""
    dist_index = frontend_dir / "dist" / "index.html"
    if not (frontend_dir / "node_modules").exists():
        subprocess.run(
            ["bun", "install", "--frozen-lockfile"], cwd=frontend_dir, check=True
        )
    newest_src = max(
        (
            p.stat().st_mtime
            for p in frontend_dir.rglob("*")
            if "node_modules" not in p.parts and "dist" not in p.parts
        ),
        default=0.0,
    )
    if not dist_index.exists() or dist_index.stat().st_mtime < newest_src:
        subprocess.run(["bun", "run", "build"], cwd=frontend_dir, check=True)
    return frontend_dir / "dist"


def _wait_for_health(url: str, timeout_s: float = 12.0) -> None:
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        try:
            if httpx.get(url, timeout=1.0).status_code == 200:
                return
        except httpx.HTTPError:
            time.sleep(0.1)
    raise TimeoutError(f"app did not become healthy at {url}")


@contextmanager
def run_app_in_thread(*, app: str, port: int) -> Iterator[str]:
    """Spawn the FastAPI app on a daemon thread (lifespan on), yield its URL."""
    config = uvicorn.Config(
        app=app,
        host="127.0.0.1",
        port=port,
        log_level="warning",
        access_log=False,
        lifespan="on",
    )
    server = uvicorn.Server(config)
    thread = threading.Thread(target=server.run, daemon=True)
    thread.start()
    url = f"http://127.0.0.1:{port}"
    try:
        _wait_for_health(f"{url}/health")
        yield url
    finally:
        server.should_exit = True
        thread.join(timeout=5)


@pytest.fixture(scope="session")
def frontend_dist() -> Path:
    return build_frontend_dist(Path(__file__).parents[3] / "frontend")


@pytest.fixture(scope="session")
def backend_url(
    postgres_container, frontend_dist, _ensure_seed_schemas, env
) -> Iterator[str]:
    # pin_return_schema(env.schema)  # match seeder writes to app reads (harness gotcha)
    with run_app_in_thread(
        app="<module>.backend.app:app", port=find_free_port()
    ) as url:
        yield url


# --- the shared browser fixtures (root-conftest plugin in the real repo) -----
@pytest_asyncio.fixture(scope="session")
async def playwright_instance() -> AsyncIterator[Playwright]:
    async with async_playwright() as pw:
        yield pw


@pytest_asyncio.fixture(scope="session")
async def browser(playwright_instance: Playwright) -> AsyncIterator[Browser]:
    headless = os.environ.get("PLAYWRIGHT_HEADED") != "1"
    b = await playwright_instance.chromium.launch(headless=headless)
    try:
        yield b
    finally:
        await b.close()


@pytest.fixture(scope="session")
def tester_email() -> str:
    return "dev@nimbus.example"


@pytest_asyncio.fixture
async def context(
    browser: Browser, backend_url: str, tester_email: str
) -> AsyncIterator[BrowserContext]:
    ctx = await browser.new_context(
        base_url=backend_url,
        extra_http_headers={"X-Auth-Email": tester_email},
    )
    try:
        yield ctx
    finally:
        await ctx.close()


@pytest_asyncio.fixture
async def page(context: BrowserContext) -> AsyncIterator[Page]:
    p = await context.new_page()
    try:
        yield p
    finally:
        await p.close()
