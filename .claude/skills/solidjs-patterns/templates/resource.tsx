// Reference resource — async data the idiomatic way.
//
// Demonstrates the patterns from references/data-async.md:
//   - createResource driven by a reactive SOURCE (not fetch-in-effect)
//   - a falsy source gates the fetch (precondition without an if)
//   - mutate() for optimistic updates, refetch() to reconcile
//   - <Suspense> for one coordinated loading state, <ErrorBoundary> for failures
//
// Confirm current options with context7: query-docs "createResource source mutate refetch Suspense".

import { createResource, createSignal, ErrorBoundary, Show, Suspense } from "solid-js";

type User = { id: number; name: string; email: string };

async function fetchUser(id: number): Promise<User> {
  const res = await fetch(`/api/users/${id}`);
  if (!res.ok) throw new Error(`User ${id}: ${res.status}`);
  return res.json();
}

export function UserCard() {
  // The source is a signal. When it changes, the resource re-fetches.
  // While userId() is falsy (null), the fetcher does NOT run — gated fetch.
  const [userId, setUserId] = createSignal<number | null>(1);

  const [user, { mutate, refetch }] = createResource(userId, fetchUser);

  // Optimistic rename: update the resource locally, then reconcile with the server.
  function rename(name: string) {
    const current = user();
    if (!current) return;
    mutate({ ...current, name }); // instant UI
    void persist(current.id, name).then(() => refetch()); // confirm + re-sync
  }

  return (
    <ErrorBoundary
      fallback={(err: Error, reset: () => void) => (
        <div role="alert">
          <p>Failed: {err.message}</p>
          <button onClick={reset}>Retry</button>
        </div>
      )}
    >
      <nav>
        <button onClick={() => setUserId(id => (id ?? 0) + 1)}>Next user</button>
        <button onClick={() => setUserId(null)}>Clear</button>
      </nav>

      {/* One fallback for the whole subtree that reads the resource */}
      <Suspense fallback={<p>Loading…</p>}>
        <Show when={user()} fallback={<p>No user selected.</p>}>
          {(u) => (
            <article>
              <h3>{u().name}</h3>
              <p>{u().email}</p>
              <button onClick={() => rename(prompt("New name?") ?? u().name)}>
                Rename
              </button>
            </article>
          )}
        </Show>
      </Suspense>
    </ErrorBoundary>
  );
}

async function persist(id: number, name: string): Promise<void> {
  await fetch(`/api/users/${id}`, {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ name }),
  });
}
