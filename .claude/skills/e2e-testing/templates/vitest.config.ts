/**
 * Vitest config for a TypeScript frontend.
 *
 * - environment: 'happy-dom' — faster than jsdom for component tests; switch to
 *   'jsdom' only if you hit a DOM-fidelity gap.
 * - setupFiles: jest-dom + axe matchers and the MSW server lifecycle (see
 *   vitest.setup.ts). Runs before every test file.
 * - include/exclude: collect *.test.ts(x) only; never pick up Playwright's
 *   e2e/*.spec.ts (a different runner owns those).
 */
import { defineConfig } from 'vitest/config'
// import react from '@vitejs/plugin-react'   // uncomment for React/JSX

export default defineConfig({
  // plugins: [react()],
  test: {
    environment: 'happy-dom',
    globals: true, // so describe/it/expect are available without imports
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    exclude: ['e2e/**', 'node_modules/**', 'dist/**'],
    css: false, // don't process CSS in unit/component tests
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html'],
      // Locate gaps, don't gate hard on a number — prefer real assertions.
      // thresholds: { lines: 0 },  // ratchet upward deliberately if you gate
    },
  },
})
