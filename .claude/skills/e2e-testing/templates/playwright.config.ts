/**
 * Playwright config for end-to-end + visual-regression tests.
 *
 * - testDir: e2e/ with *.spec.ts (Vitest owns *.test.ts elsewhere).
 * - webServer: builds + serves the app, so E2E runs against the real bundle.
 * - projects: one per browser; the PR `e2e` task narrows to @smoke (see the
 *   e2e-visual reference), the nightly task runs the full matrix.
 * - toHaveScreenshot: deterministic visual diffs (animations disabled, small
 *   pixel tolerance). Commit the baselines; pin @playwright/test so renders are
 *   stable across machines.
 */
import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  testMatch: '**/*.spec.ts',
  fullyParallel: true,
  forbidOnly: !!process.env.CI, // a stray test.only fails CI
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? [['html'], ['github']] : 'list',

  use: {
    baseURL: 'http://localhost:4173',
    trace: 'on-first-retry', // a trace to debug the flake, not on every run
  },

  expect: {
    toHaveScreenshot: { maxDiffPixelRatio: 0.01, animations: 'disabled' },
  },

  // Build once, serve, and reuse locally; CI gets a fresh server each run.
  webServer: {
    command: 'npm run build && npm run preview',
    url: 'http://localhost:4173',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },

  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
    { name: 'firefox', use: { ...devices['Desktop Firefox'] } },
    { name: 'webkit', use: { ...devices['Desktop Safari'] } },
    { name: 'mobile', use: { ...devices['Pixel 7'] } },
  ],
})
