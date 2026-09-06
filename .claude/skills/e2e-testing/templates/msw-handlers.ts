/**
 * MSW request handlers — the single mocked seam for the whole test suite.
 *
 * These same handlers feed Vitest (via setupServer in vitest.setup.ts) and can
 * feed the browser (via setupWorker) for Playwright or local dev. Keep one source
 * of truth: don't fork mock data per test file. Shape payloads like the real API
 * — ideally typed from the same contract — so the mock can't drift from prod.
 *
 * MSW v2 syntax: `http` + `HttpResponse`.
 */
import { http, HttpResponse } from 'msw'

export const handlers = [
  http.get('/api/cart', () =>
    HttpResponse.json({
      items: [{ sku: 'A', price: 50, qty: 2 }],
      total: 100,
    }),
  ),

  http.post('/api/orders', async ({ request }) => {
    const body = (await request.json()) as { email?: string }
    if (!body.email) {
      return HttpResponse.json({ error: 'email required' }, { status: 422 })
    }
    return HttpResponse.json({ id: 'ord_1', status: 'confirmed' }, { status: 201 })
  }),
]
