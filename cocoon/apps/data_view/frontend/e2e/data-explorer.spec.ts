import { test, expect, type ConsoleMessage, type Page } from "@playwright/test";

// Data Explorer: loads real rows, and switching tables persists in the URL and
// survives a reload — without depending on any specific seeded table name.

function trackErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("console", (m: ConsoleMessage) => {
    if (m.type() === "error") errors.push(m.text());
  });
  page.on("pageerror", (e) => errors.push(String(e)));
  return errors;
}

// The toolbar selects, in DOM order: schema (0), table (1), timestamp (2).
const tableSelect = (page: Page) =>
  page.locator('[data-slot="select-trigger"]').nth(1);

/** Open the table select and click the first option that isn't `current`. */
async function pickDifferentTable(page: Page, current: string): Promise<void> {
  await tableSelect(page).click();
  const options = page.getByRole("option");
  await options.first().waitFor({ timeout: 10_000 });
  const count = await options.count();
  for (let i = 0; i < count; i++) {
    const text = (await options.nth(i).innerText()).trim();
    if (text && text !== current) {
      await options.nth(i).click();
      return;
    }
  }
  throw new Error("no alternative table option found");
}

test("loads with real data, does not freeze, logs no console errors", async ({
  page,
}) => {
  const errors = trackErrors(page);

  await page.goto("/data-explorer");

  // Real rows arrived (the row-count line only renders once data is in).
  await expect(page.getByText(/Showing [\d,]+ rows/)).toBeVisible({
    timeout: 15_000,
  });

  expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toEqual([]);
});

test("table Select opens, switches without vanishing, and persists across reload", async ({
  page,
}) => {
  const errors = trackErrors(page);

  await page.goto("/data-explorer");
  await expect(page.getByText(/Showing [\d,]+ rows/)).toBeVisible({
    timeout: 15_000,
  });
  // Wait for the table select to settle on a real table (not the placeholder),
  // otherwise we'd "switch" to the already-selected default and Radix fires no
  // change event.
  await expect(tableSelect(page)).not.toHaveText("Select a table", {
    timeout: 15_000,
  });

  const before = (await tableSelect(page).innerText()).trim();
  await pickDifferentTable(page, before);

  // The trigger is still mounted and now shows a different table — no teardown.
  await expect(tableSelect(page)).not.toHaveText(before);
  // The pick is written to the URL.
  await expect(page).toHaveURL(/[?&]table=/);

  // A refresh restores the selection from the URL.
  const switched = (await tableSelect(page).innerText()).trim();
  await page.reload();
  await expect(tableSelect(page)).toHaveText(switched, { timeout: 15_000 });
  await expect(page).toHaveURL(/[?&]table=/);

  expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toEqual([]);
});
