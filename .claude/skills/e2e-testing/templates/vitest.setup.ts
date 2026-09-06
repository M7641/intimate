/**
 * Global test setup, loaded before every test file (see vitest.config.ts).
 *
 * Wires three things:
 *  1. jest-dom matchers (toBeInTheDocument, toHaveTextContent, …)
 *  2. vitest-axe matchers (toHaveNoViolations)
 *  3. the MSW server lifecycle — start once, reset handlers after each test,
 *     close at the end. `onUnhandledRequest: 'error'` makes an un-mocked request
 *     fail the test instead of hanging silently.
 */
import { afterAll, afterEach, beforeAll, expect } from 'vitest'
import '@testing-library/jest-dom/vitest'
import * as axeMatchers from 'vitest-axe/matchers'
import { setupServer } from 'msw/node'

import { handlers } from './msw/handlers'

expect.extend(axeMatchers)

export const server = setupServer(...handlers)

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterEach(() => server.resetHandlers())
afterAll(() => server.close())

// Note: @testing-library/react auto-cleans the DOM after each test when Vitest's
// `globals: true` is set, so no manual cleanup() is needed.
