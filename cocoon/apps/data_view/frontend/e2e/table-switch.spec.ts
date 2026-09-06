import { test, expect, type ConsoleMessage, type Route, type Page } from "@playwright/test";

// Regression guard for "the page wipes when I pick a new table". The bug: the
// query went `pending` with no data on a switch, suspended/blanked, and the
// toolbar disappeared for the load window. A switch must keep the chrome up and
// swap rows in place. Seed-agnostic: it switches to whatever the second table is.

const slow = (ms: number) => async (route: Route) => {
  await new Promise((r) => setTimeout(r, ms));
  await route.continue();
};

const tableSelect = (page: Page) =>
  page.locator('[data-slot="select-trigger"]').nth(1);

test("switching tables keeps the page up and swaps data in place", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("console", (m: ConsoleMessage) => {
    if (m.type() === "error") errors.push(m.text());
  });
  page.on("pageerror", (e) => errors.push(String(e)));

  // Latency makes the load window observable; locally the backend is too fast
  // to catch a transient blank.
  await page.route("**/api/data_view/timestamps/**", slow(500));
  await page.route("**/api/data_view/data/**", slow(500));

  await page.goto("/data-explorer");
  await expect(page.getByText(/Showing [\d,]+ rows/)).toBeVisible({
    timeout: 15_000,
  });
  // Wait for a real table (not the placeholder) before switching, else Radix
  // sees no change when we "pick" the already-selected default.
  await expect(tableSelect(page)).not.toHaveText("Select a table", {
    timeout: 15_000,
  });

  const before = (await tableSelect(page).innerText()).trim();

  // Open the table select and pick the first option that isn't current.
  await tableSelect(page).click();
  const options = page.getByRole("option");
  await options.first().waitFor({ timeout: 10_000 });
  const count = await options.count();
  for (let i = 0; i < count; i++) {
    const text = (await options.nth(i).innerText()).trim();
    if (text && text !== before) {
      await options.nth(i).click();
      break;
    }
  }

  // Sample the toolbar across the whole load window: it must never vanish.
  let minTriggers = Infinity;
  for (let i = 0; i < 8; i++) {
    const triggers = await page
      .locator('[data-slot="select-trigger"]')
      .count();
    minTriggers = Math.min(minTriggers, triggers);
    await page.waitForTimeout(120);
  }
  expect(
    minTriggers,
    "the page blanked during the switch (toolbar disappeared)",
  ).toBeGreaterThan(0);

  // The switch completed: a different table is shown and written to the URL.
  await expect(tableSelect(page)).not.toHaveText(before);
  await expect(page).toHaveURL(/[?&]table=/);

  // A refresh restores it.
  const switched = (await tableSelect(page).innerText()).trim();
  await page.reload();
  await expect(tableSelect(page)).toHaveText(switched, { timeout: 15_000 });
  await expect(page).toHaveURL(/[?&]table=/);

  expect(errors, `console errors:\n${errors.join("\n")}`).toEqual([]);
});
