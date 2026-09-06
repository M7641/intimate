# Async data: createResource, Suspense, ErrorBoundary

**API recap (verify with context7 → `query-docs "createResource Suspense ErrorBoundary"`)**

```tsx
const [data, { refetch, mutate }] = createResource(source, fetcher);
//                                   ^source     ^ (sourceValue) => Promise
data();          // current value (undefined while first loading)
data.loading;    // boolean
data.error;      // thrown error, if any
```

## The source-driven pattern (avoid manual fetch-in-effect)

Don't fetch inside a `createEffect` and stuff the result in a signal — you'll fight
race conditions and double-fetches. `createResource` is built for this: give it a
**reactive source**, and it re-fetches when the source changes and tracks loading.

```tsx
const [userId, setUserId] = createSignal(1);
const [user] = createResource(userId, (id) => fetchUser(id)); // re-fetches on id change
```

If the source is `false`/`null`/`undefined`, the fetcher **does not run** — that's
the idiomatic way to gate a fetch on a precondition.

## Suspense for loading UI

Wrap resource-reading UI in `<Suspense>` to get one coordinated fallback instead of
per-component spinners:

```tsx
<Suspense fallback={<Spinner />}>
  <Profile user={user()} />
</Suspense>
```

`<ErrorBoundary>` does the same for thrown errors:

```tsx
<ErrorBoundary fallback={(err, reset) => <Failed error={err} retry={reset} />}>
  <Suspense fallback={<Spinner />}><Profile /></Suspense>
</ErrorBoundary>
```

## Pitfalls

- **Flashing / double fetch** — usually a fetch-in-effect that should be a resource,
  or a source that changes identity every render (memoize the source).
- **Never resolves** — the source is stuck falsy, so the fetcher is gated off; check
  what you pass as the first arg.
- **Optimistic updates** — use the returned `mutate(value)` to set the resource
  locally, then `refetch()` to reconcile with the server.

→ context7: `query-docs "createResource source mutate refetch Suspense"` for the
current options object and SSR-streaming behaviour.
