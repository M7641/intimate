# Reactivity: signals, props, and tracking scope

This is the root of most Solid bugs. Get this page right and the others are detail.

**API recap (verify current signatures with context7 → `query-docs "createSignal"`)**

```ts
const [count, setCount] = createSignal(0); // read with count(), write with setCount
count();            // a *read* — only subscribes if inside a tracking scope
setCount(c => c + 1);
```

A **tracking scope** is: JSX, `createEffect`, `createMemo`, and helpers like
`createResource`. A signal read *inside* one subscribes that scope to the signal.
A read *outside* one is a plain snapshot that never updates.

## Pitfall 1 — destructuring props kills reactivity

`props` is a **proxy of getters**. Destructuring evaluates each getter *once*, at
that instant, and throws the proxy away. The value can never update again.

```tsx
// ✗ broken — `name` is frozen at first render
function Hello({ name }: { name: string }) {
  return <p>Hi {name}</p>;
}

// ✓ keep the proxy — read props.name inside the JSX tracking scope
function Hello(props: { name: string }) {
  return <p>Hi {props.name}</p>;
}
```

Need to pull fields out or set defaults? Use the helpers, which preserve getters:

```tsx
const [local, others] = splitProps(props, ["name"]);   // local.name stays reactive
const merged = mergeProps({ size: "md" }, props);       // reactive defaults
```

## Pitfall 2 — reading a signal into a plain variable too early

```tsx
function Counter() {
  const [count, setCount] = createSignal(0);
  const value = count();            // ✗ snapshot taken once, in the body
  return <button onClick={() => setCount(count() + 1)}>{value}</button>;
  //                                                     ^ never updates
}
```

The body runs **once**, so `value` is fixed forever. Read the signal *inside* the
tracking scope instead — call it where it's used:

```tsx
return <button onClick={() => setCount(c => c + 1)}>{count()}</button>;
//                                                   ^ JSX tracks this read
```

Rule of thumb: **pass the function, don't pre-call it.** `{count()}` in JSX is fine
(JSX is a tracking scope); `const v = count()` in the body is not.

## Pitfall 3 — losing reactivity when spreading

Spreading a derived object can evaluate getters eagerly. When forwarding props,
prefer `mergeProps` / `splitProps` over `{...props}` gymnastics, and avoid building
intermediate plain objects from reactive sources.

## Escape hatches (use sparingly, know why)

- `untrack(() => count())` — read without subscribing.
- `batch(() => { setA(1); setB(2); })` — coalesce writes into one update.
- `on(count, () => ...)` — make an effect's dependencies **explicit** instead of
  auto-tracked (also `{ defer: true }` to skip the first run).

→ For exact signatures and newer options, context7: `query-docs "createSignal untrack batch on"`.
