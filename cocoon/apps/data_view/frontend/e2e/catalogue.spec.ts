import { test, expect, type ConsoleMessage, type Page } from "@playwright/test";

// Regression guard for the catalogue (relationship-graph) page. The bug:
// Cytoscape was initialised while its container was still 0×0 — onMount runs
// before the browser lays out the flex — so the cose layout read an undefined
// bounding box ("Cannot read properties of undefined (reading 'w')"), and the
// throw bubbled to the router's match boundary ("Error in route match:
// __root__"), blanking the page. The fix sizes the graph (cy.resize) in a rAF
// and guards the build. These tests assert the page renders a *sized* graph
// with no errors.

/** Collect console errors + uncaught page errors for the life of the test. */
function trackErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("console", (m: ConsoleMessage) => {
    if (m.type() === "error") errors.push(m.text());
  });
  page.on("pageerror", (e) => errors.push(String(e)));
  return errors;
}

test("catalogue page renders a sized graph with no errors", async ({
  page,
}) => {
  const errors = trackErrors(page);

  await page.goto("/catalogue");

  // The schema selector is the page chrome; it must appear and stay up (a
  // route-match blank would drop it).
  await expect(
    page.locator('[data-slot="select-trigger"]').first(),
  ).toBeVisible({ timeout: 15_000 });

  // The summary count proves the /catalogue endpoint resolved and rendered.
  await expect(page.getByText(/\d+ tables/)).toBeVisible({ timeout: 15_000 });

  // Cytoscape renders into <canvas> layers inside the graph container.
  const canvas = page.locator("canvas").first();
  await expect(canvas).toBeVisible({ timeout: 15_000 });

  // The container had a real size when the graph was built (the resize fix);
  // a 0×0 canvas is exactly the bug's signature.
  const box = await canvas.boundingBox();
  expect(box, "graph canvas has no layout box").not.toBeNull();
  expect(box!.width, "graph canvas has zero width").toBeGreaterThan(0);
  expect(box!.height, "graph canvas has zero height").toBeGreaterThan(0);

  // ...and something was actually drawn. Cytoscape's layer canvases are
  // transparent, so any non-zero alpha pixel means nodes/edges rendered — this
  // is what catches the "sized but blank" regression (the graph never fit
  // because the container was 0×0 when it was first laid out). Poll because the
  // ResizeObserver builds the graph a beat after the canvas appears.
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          for (const c of Array.from(document.querySelectorAll("canvas"))) {
            const ctx = c.getContext("2d");
            if (!ctx || !c.width || !c.height) continue;
            const { data } = ctx.getImageData(0, 0, c.width, c.height);
            for (let i = 3; i < data.length; i += 4 * 37) {
              if (data[i] !== 0) return true; // a drawn (opaque) pixel
            }
          }
          return false;
        }),
      { message: "graph canvas is blank — nothing was drawn", timeout: 15_000 },
    )
    .toBe(true);

  // No "reading 'w'" crash, no route-match blank.
  expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toEqual([]);
});

test("catalogue is reachable from the nav and keeps its chrome", async ({
  page,
}) => {
  const errors = trackErrors(page);

  await page.goto("/");
  await page.getByRole("link", { name: "Catalogue" }).click();
  await expect(page).toHaveURL(/\/catalogue/);

  // The graph mounts on the freshly-navigated route (not just a hard load).
  await expect(page.locator("canvas").first()).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText(/\d+ tables/)).toBeVisible({ timeout: 15_000 });

  expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toEqual([]);
});
