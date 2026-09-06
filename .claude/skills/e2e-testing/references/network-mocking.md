# Mocking the network with MSW (the one seam you mock)

MSW (Mock Service Worker) intercepts HTTP at the network layer — the same requests
the browser would make. This is the *only* thing we mock in component/integration
tests: not `fetch`, not axios, not our own modules. The payoff is honesty (the app
runs its real data code) and refactor-resistance (rename a function, the test
still passes; change the API contract, it fails — which is correct).

Same handlers power both worlds: Node (Vitest, via `setupServer`) and the browser
(Playwright/dev, via `setupWorker`). Write them once.

## Define handlers (MSW v2 syntax)

```ts
// src/test/msw/handlers.ts
import { http, HttpResponse } from 'msw'

export const handlers = [
  http.get('/api/cart', () =>
    HttpResponse.json({ items: [{ sku: 'A', price: 50, qty: 2 }], total: 100 })),

  http.post('/api/orders', async ({ request }) => {
    const body = (await request.json()) as { email: string }
    if (!body.email) {
      return HttpResponse.json({ error: 'email required' }, { status: 422 })
    }
    return HttpResponse.json({ id: 'ord_1', status: 'confirmed' }, { status: 201 })
  }),
]
```

## Server lifecycle (Vitest)

Start once, **reset after every test** (so one test's overrides don't leak), close
at the end. This lives in the Vitest setup file:

```ts
// src/test/setup.ts (excerpt — see templates/vitest.setup.ts for the full file)
import { afterAll, afterEach, beforeAll } from 'vitest'
import { setupServer } from 'msw/node'
import { handlers } from './msw/handlers'

export const server = setupServer(...handlers)

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterEach(() => server.resetHandlers())
afterAll(() => server.close())
```

`onUnhandledRequest: 'error'` is deliberate: a request with no handler **fails the
test**. That's how you catch the app calling an endpoint you forgot to mock,
instead of a silent hang.

## Per-test overrides (error & edge states)

Test the unhappy paths by overriding a handler for one test with `server.use(...)`.
`resetHandlers()` (above) undoes it afterwards:

```tsx
import { http, HttpResponse } from 'msw'
import { server } from '../test/setup'

it('shows an error when the order fails', async () => {
  server.use(
    http.post('/api/orders', () =>
      HttpResponse.json({ error: 'card declined' }, { status: 402 })),
  )
  const user = userEvent.setup()
  render(<CheckoutForm />)
  await user.click(screen.getByRole('button', { name: /place order/i }))
  expect(await screen.findByRole('alert')).toHaveTextContent(/card declined/i)
})
```

Always cover the three states of any network-backed UI: **loading**, **success**,
**error**. Loading-state assertions need either a deferred response or fake timers
so the pending UI is observable before resolution.

## Rules

- **One source of truth for handlers** — the same `handlers.ts` seeds Vitest,
  Playwright, and (optionally) the dev server. Don't fork mock data per test file.
- **Keep mock payloads close to real** — shape them like the real API (ideally
  derived from the same types) so the mock can't drift from production.
- **Never assert on the request from inside the component test** unless the
  request *is* the behaviour (e.g. analytics). Assert on what the user sees.
- **For E2E**, prefer the real (or a staging) backend; reach for MSW in the browser
  only to stub third-party calls you don't control.
