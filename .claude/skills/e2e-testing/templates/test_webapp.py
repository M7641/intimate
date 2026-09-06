"""A seeded, query-by-role webapp E2E spec.

Depends on the browser `page` fixture (this skill's conftest) and seeder fixtures
(postgres-test-harness). The harness auto-truncates seed tables before each test,
so every test builds its own world from scratch.
"""

from __future__ import annotations

from playwright.async_api import Page, expect

from .conftest import realistic_week_rows
from .seeds import PermissionsUsersSeeder, OrdersTotalSeeder

# The app boots a real backend, so first render is slow — wait, never sleep.
FIRST_RENDER_TIMEOUT_MS = 15_000


async def test_01_01_landing_page_shows_logo(page: Page) -> None:
    """The logo identifies the platform on the root URL."""
    await page.goto("/")
    await expect(page.get_by_alt_text("Acme")).to_be_visible(
        timeout=FIRST_RENDER_TIMEOUT_MS
    )


async def test_02_01_past_and_future_rows_render(
    page: Page,
    permissions_users: PermissionsUsersSeeder,
    orders_total: OrdersTotalSeeder,
) -> None:
    """A tab loaded with past + future weeks renders a grid with both."""
    await permissions_users.add(
        email="dev@nimbus.example", account_manager="AM", category_code="10"
    )
    for row in realistic_week_rows(n_past=2, n_future=2):
        await orders_total.add(**row)

    await page.goto("/orders")

    # a rendered currency cell proves rows loaded AND the formatter ran
    await expect(page.get_by_text("£100,000").first).to_be_visible()


async def test_02_02_empty_universe_shows_empty_state(
    page: Page, permissions_users
) -> None:
    """A user with no permitted categories sees the empty state, not an error."""
    await permissions_users.add(
        email="dev@nimbus.example", account_manager="AM", category_code="99"
    )
    await page.goto("/orders")
    await expect(
        page.get_by_role("status", name=lambda n: "no data" in n.lower())
    ).to_be_visible()
