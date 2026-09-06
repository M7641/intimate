# End-to-end, visual regression, browser a11y, and Lighthouse (Playwright)

Playwright is the top of the trophy: a real browser driving the *built* app over
the critical user journeys. It also hosts visual-regression (`toHaveScreenshot`)
and browser-level accessibility (`@axe-core/playwright`). Keep this layer thin and
deterministic — it's the slowest and flakiest, so most of it runs nightly.

## End-to-end specs

Live in `e2e/*.spec.ts` (the `.spec` suffix keeps them out of Vitest's collection).
Drive the app the way a user would, query by role, assert on user-visible outcomes:

```ts
// e2e/checkout.spec.ts
import { test, expect } from '@playwright/test'

test('a shopper can place an order', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Add to cart' }).first().click()
  await page.getByRole('link', { name: 'Checkout' }).click()
  await page.getByRole('textbox', { name: 'Email' }).fill('a@b.com')
  await page.getByRole('button', { name: 'Place order' }).click()
  await expect(page.getByRole('status')).toHaveText(/order confirmed/i)
})
```

Determinism rules: rely on Playwright's **auto-waiting** (web-first assertions
retry) — never `waitForTimeout`. Pin time/locale/seeded data. Tag the must-pass
subset (`@smoke`) so the PR gate runs only those and the nightly job runs the rest.

```ts
test('sign-in @smoke', async ({ page }) => { /* ... */ })
```

## Visual regression

Screenshot a stable component or page and diff against a committed baseline. Mask
or freeze anything non-deterministic (timestamps, avatars, animations):

```ts
test('checkout page looks right', async ({ page }) => {
  await page.goto('/checkout')
  await expect(page).toHaveScreenshot('checkout.png', {
    maxDiffPixelRatio: 0.01,
    mask: [page.getByTestId('current-time')],
    animations: 'disabled',
  })
})
```

Workflow: generate baselines with `playwright test --update-snapshots`, **commit**
them, and pin the browser version (the Playwright dependency version) so renders
don't drift across machines. Baselines are render-environment-sensitive — generate
them on (or matched to) the CI image, exactly as the backend benchmark baseline is
matched to the CI runner. Component-level snapshots are fast and deterministic
enough to gate on a PR; the full page suite runs nightly.

`Chromatic` (with Storybook) is the managed alternative — it renders stories in a
hosted, pinned environment and reviews diffs on the PR. Reach for it when the
visual surface is large enough that maintaining committed PNGs hurts; otherwise
committed Playwright screenshots keep it in-repo with no external service.

## Browser-level accessibility

Component tests catch most a11y issues via `vitest-axe`; the E2E layer scans the
*fully assembled, real-CSS* page where contrast and focus order actually live:

```ts
import AxeBuilder from '@axe-core/playwright'

test('checkout has no a11y violations', async ({ page }) => {
  await page.goto('/checkout')
  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa'])
    .analyze()
  expect(results.violations).toEqual([])
})
```

## Lighthouse budgets

Lighthouse CI (`@lhci/cli`) turns performance, accessibility, and best-practices
into asserted budgets — the runtime analogue of the bundle-size gate. It's slower
and a touch noisy, so run it nightly against a preview/built deploy and ratchet the
floors:

```js
// lighthouserc.js
export default {
  ci: {
    collect: { url: ['http://localhost:4173/'], numberOfRuns: 3 },
    assert: {
      assertions: {
        'categories:performance': ['error', { minScore: 0.9 }],
        'categories:accessibility': ['error', { minScore: 1 }],
        'largest-contentful-paint': ['error', { maxNumericValue: 2500 }],
        'total-blocking-time': ['error', { maxNumericValue: 200 }],
        'cumulative-layout-shift': ['error', { maxNumericValue: 0.1 }],
      },
    },
  },
}
```

## moon tasks

```yaml
tasks:
  e2e:                          # smoke on PR, full matrix nightly
    command: 'playwright test --grep @smoke'
    toolchain: 'node'
    deps: ['~:install', '~:install-browsers', '~:build']
    options: { cache: false }
  e2e-full:                     # nightly: every browser/viewport, visual suite
    command: 'playwright test'
    toolchain: 'node'
    deps: ['~:install', '~:install-browsers', '~:build']
    options: { cache: false, runInCI: false }
  install-browsers:
    command: 'playwright install --with-deps'
    toolchain: 'node'
    options: { cache: false }
  bundlesize:                   # PR gate: fails if a chunk exceeds its budget
    command: 'size-limit'
    toolchain: 'node'
    deps: ['~:build']
  lighthouse:                   # nightly against a preview deploy
    command: 'lhci autorun'
    toolchain: 'node'
    deps: ['~:build']
    options: { cache: false, runInCI: false }
```

CI runs Playwright in its official container or after `playwright install
--with-deps`; the browsers are not npm packages, so the `install-browsers` dep is
what makes E2E reproducible. Pin the `@playwright/test` version so the bundled
browser builds — and therefore the screenshots — are stable.
