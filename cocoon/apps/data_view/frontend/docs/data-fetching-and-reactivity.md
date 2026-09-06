# Data fetching & loading states

How pages read server data without flashing a blank screen on every refetch.
Read this before adding a page or a `useQuery` hook. (We use React +
`@tanstack/react-query`; a short note on the SolidJS spell this was learned
during is at the end.)

The short rule:

> Gate the **skeleton** on _nothing-yet_ (`!data && !error`), gate the
> **content** on _data presence_ (`{data && …}`), and give every selector-driven
> query `placeholderData: keepPreviousData`. Never gate a subtree on
> `!isPending`.

---

## The anti-pattern

A page shows a skeleton while pending and gates all its content on `!isPending`:

```tsx
const health = useSchemaHealth(selectedSchema); // no placeholderData

<DataLoading isPending={health.isPending} ... />
{!health.isPending && !health.isError && summary && (
  <>{/* cards + table */}</>
)}
```

The symptom: changing the schema in the selector **blanks the content** — on
each switch the query goes empty-and-pending, the content unmounts, the skeleton
flashes, and scroll position is lost. The click feels like the page broke.

## What we changed it to

```tsx
const health = useSchemaHealth(selectedSchema); // placeholderData: keepPreviousData
// Skeleton only on the very first load; refetches keep the old data on screen.
// "First load" = nothing to show yet, NOT `isPending` — see the caution below.
const firstLoad = !health.data && !health.error;

<DataLoading isPending={firstLoad} error={health.error} ... />
{summary && (
  <>{/* cards + table — stays on screen across refetches, updates in place */}</>
)}
```

And in the query hook:

```ts
export function useSchemaHealth(schema: string) {
  return useQuery<SchemaHealth>({
    queryKey: ["schemaHealth", schema],
    queryFn: ...,
    enabled: !!schema,
    placeholderData: keepPreviousData, // <- the key line
  });
}
```

## Why it works like this

`@tanstack/react-query` keys a query by its `queryKey`. When `schema` changes
the key changes, so react-query treats it as a **new** query:

1. **Without `keepPreviousData`**, the new key has no cached data, so `data`
   becomes `undefined` and `isPending` flips back to `true` for the load window.
   Anything gated on `data` (or on `!isPending`) unmounts — the content
   disappears and the skeleton flashes on every switch.

2. **With `placeholderData: keepPreviousData`**, react-query keeps serving the
   _previous_ key's data while the new one loads. `data` stays defined the whole
   time, so the content never unmounts. During this window `isPending` is
   `false` and `isFetching`/`isPlaceholderData` are `true` — which is exactly why
   gating on `!isPending` is wrong: with `keepPreviousData` it stays `false`
   (no skeleton, good) but without it, it briefly flips `true` and tears the
   subtree down.

The component itself never unmounts on a refetch — your `useState` selectors and
scroll position survive, and react-query just hands the component new `data` to
render. Keep state in the component (or the URL) so it outlives any one fetch,
and let the view stay mounted and swap values in place.

### The mental model

- **Skeleton** answers "do we have _anything_ to show yet?" → `!data && !error`.
- **Content** answers "do we have data _now_ (even stale)?" → `{data && …}`.
- **`keepPreviousData`** is what makes "stale data now" possible, so the two
  questions have different answers during a refetch and the page never blanks.

> **Caution — don't use `isPending` for the skeleton.** Our queries are gated
> with `enabled: !!schema`, and a *disabled* query reports `isPending: true`
> with no data — so `isPending && !data` is true before a schema is even picked,
> yet `isPending`'s exact semantics for disabled/placeholder states are subtle.
> `!data && !error` says exactly what the skeleton means — "nothing yet" — and
> is true through the disabled phase, the first fetch, and nothing else.

## Showing that a refetch is in flight

`keepPreviousData` fixes the blanking but creates a new gap: when you change a
selector, the query refetches for several seconds while the **old** data sits on
screen. Without a cue, the click looks like it did nothing. The cue is the third
loading state — distinct from the first-load skeleton:

| State             | Condition            | Show                                   |
| ----------------- | -------------------- | -------------------------------------- |
| First load        | `!data && !error`    | full skeleton (`<DataLoading>`)        |
| Refetch in flight | `isFetching && data` | keep stale data + a subtle "Updating…" |
| Idle              | `data && !isFetching`| the data, plainly                      |

`isPending`, `isFetching` and `data` come off the query result; read them in
render and React re-renders the component when they change:

```tsx
const firstLoad = !q.data && !q.error;       // skeleton: nothing yet
const updating  = q.isFetching && !!q.data;  // in-flight cue

<DataLoading isPending={firstLoad} error={q.error} />
<FetchingIndicator when={updating} />            {/* spinner + "Updating…" */}
<div className={cn("…", updating && "opacity-60 transition-opacity")}>
  {/* stale-but-visible content; dimming says "refreshing, not frozen" */}
</div>
```

`@/components/FetchingIndicator` is the shared affordance. Dimming the content
container (`opacity-60`) plus the spinner is enough; don't tear the content down
— that would defeat the point of `keepPreviousData`.

All pages follow the skeleton/keep-previous pattern: `data-explorer`,
`schema-health`, `catalogue`, `table-info`, and `column-analysis` (whose
`useQueries` compare arms inherit `keepPreviousData` through
`columnStatsQueryOptions` / `valueDistributionQueryOptions`). The in-flight cue
is wired on the pages with a slow, selector-driven refetch (`schema-health`,
`catalogue`); add it anywhere a refetch is slow enough to feel like a freeze.

---

## History: this was first hit (and worse) under SolidJS

These patterns were worked out during a spell when the frontend was on SolidJS
+ `@tanstack/solid-query`, and the bug was more violent there. In solid-query a
query is backed by a Solid **resource**: reading `data` while pending **throws a
promise**, and TanStack Router wraps each route in a `<Suspense>` boundary that
**caught the throw and re-mounted the whole page** — so the selectors' signals
were recreated and reset to their defaults, not just the content. `keepPreviousData`
fixed it there by keeping the resource non-pending so the read never threw.

The React port (plain `useQuery`, no `useSuspenseQuery`) doesn't suspend on
read, so the component stays mounted and the failure is the milder content-flash
above. The fix and the three-state model are identical because react-query's
`keepPreviousData`, `isPending`, `isFetching` and `data` mean the same thing in
both adapters. (If you ever switch a query to `useSuspenseQuery`, the Solid-style
"whole boundary blanks" behaviour returns — keep `keepPreviousData` on it.)
