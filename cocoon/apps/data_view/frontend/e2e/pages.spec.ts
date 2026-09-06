import { test, expect, type ConsoleMessage, type Page } from "@playwright/test";

// Per-page smoke coverage: every page loads against the seeded stack, renders
// its signature content, and logs no console/page errors. The data-explorer and
// catalogue pages have their own dedicated specs with deeper assertions.

function trackErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("console", (m: ConsoleMessage) => {
    if (m.type() === "error") errors.push(m.text());
  });
  page.on("pageerror", (e) => errors.push(String(e)));
  return errors;
}

const noErrors = (errors: string[]) =>
  expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toEqual(
    [],
  );

test("home renders the launchpad and recent loads", async ({ page }) => {
  const errors = trackErrors(page);
  await page.goto("/");

  await expect(page.getByText("Data Views")).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText("Recently updated")).toBeVisible();
  // The nav exposes every page.
  await expect(
    page.getByRole("link", { name: "Schema Health" }),
  ).toBeVisible();

  noErrors(errors);
});

test("table-info shows overview cards and the row-count chart", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await page.goto("/table-info");

  // Auto-selects schema "stage" + first table, then renders the overview.
  await expect(page.getByText("Columns").first()).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByText("Rows (Latest)")).toBeVisible();
  await expect(page.getByText("Snapshots").first()).toBeVisible();
  // The row-counts chart is a canvas.
  await expect(page.locator("canvas").first()).toBeVisible({ timeout: 15_000 });

  noErrors(errors);
});

test("column-analysis shows column stats", async ({ page }) => {
  const errors = trackErrors(page);
  await page.goto("/column-analysis");

  // Auto-selects schema/table/column, then renders the stats grid.
  await expect(page.getByText("Distinct").first()).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByText("Nulls").first()).toBeVisible();

  noErrors(errors);
});

test("schema-health shows the summary and a per-table row", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await page.goto("/schema-health");

  // Summary StatCards.
  await expect(page.getByText("Tables").first()).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByText("Stale").first()).toBeVisible();
  await expect(page.getByText("Empty").first()).toBeVisible();
  // The health table header.
  await expect(
    page.getByRole("columnheader", { name: "Last loaded" }),
  ).toBeVisible({ timeout: 15_000 });

  noErrors(errors);
});
