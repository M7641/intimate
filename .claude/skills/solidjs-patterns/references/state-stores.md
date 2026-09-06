# State: signals vs stores

**API recap (verify with context7 → `query-docs "createStore produce reconcile"`)**

```ts
const [state, setState] = createStore({ user: { name: "A" }, todos: [] });
state.user.name;                 // read — fine-grained, tracks just this leaf
setState("user", "name", "B");   // path-based update, only that node notifies
```

## When to use which

- **`createSignal`** — a single, mostly-atomic value (a number, a string, a boolean,
  one object you always replace wholesale).
- **`createStore`** — nested/structured state where you want **fine-grained** updates:
  changing `state.user.name` should not invalidate readers of `state.todos`. Stores
  are proxies that track per leaf.

If you find yourself spreading a signal's object to update one field on every
change, that's the signal-to-store smell.

## Pitfall 1 — mutating instead of using the setter

The store proxy is **read-only**. Direct mutation changes nothing reactive:

```ts
state.user.name = "B";                 // ✗ no notification
setState("user", "name", "B");         // ✓
```

## Pitfall 2 — replacing the whole proxy

```ts
setState({ user: { name: "B" }, todos: [] });   // ✗ often blows away granularity
```

Update *paths*, not the whole tree. For complex local edits use `produce` (an
Immer-style mutable draft that stays fine-grained):

```ts
import { produce } from "solid-js/store";
setState(produce(s => { s.todos.push({ id, done: false }); }));
```

## Pitfall 3 — replacing from server data loses identity

Overwriting a store with a freshly-fetched object re-creates every node, so the UI
re-renders everything and loses DOM state. Use `reconcile` to **diff** new data into
the existing tree, keyed by id:

```ts
import { reconcile } from "solid-js/store";
setState("todos", reconcile(freshTodos, { key: "id" }));
```

This is the store equivalent of choosing the right key in `<For>` — same idea,
preserve identity across updates.

## Array updates

Path syntax targets array items by index or with a predicate:

```ts
setState("todos", t => t.id === id, "done", true);   // update matching item
```

→ context7: `query-docs "createStore array update reconcile produce"` for the full
update-grammar and current options.
