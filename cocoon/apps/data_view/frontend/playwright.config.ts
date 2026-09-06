import { defineConfig, devices } from "@playwright/test";

// E2E runs against a real, running stack: the data_view server (serving the
// built SPA) backed by a seeded Postgres container. Point E2E_BASE_URL at it.
export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: process.env.E2E_BASE_URL ?? "http://localhost:8050",
    headless: true,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
