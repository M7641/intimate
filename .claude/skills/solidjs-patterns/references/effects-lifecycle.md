# Effects, memos, and lifecycle

**API recap (verify with context7 → `query-docs "createEffect createMemo onMount onCleanup"`)**

```ts
createMemo(() => a() + b());       // cached derived value; re-runs when a/b change
createEffect(() => console.log(count()));  // side effect; re-runs on tracked reads
onMount(() => { /* runs once, after first render */ });
onCleanup(() => { /* runs on disposal / before re-run */ });
```

## Memo vs effect — pick by intent

- **`createMemo`** — you want a **value** derived from other reactive sources, cached
  and only recomputed when inputs change. Read it like a signal: `total()`.
- **`createEffect`** — you want a **side effect** (logging, imperative DOM, syncing to
  a non-reactive system). It returns nothing useful.

Computing a value inside an effect and storing it in a signal is almost always a
memo in disguise — and it adds a wasted render pass.

```ts
// ✗ effect-as-memo: extra pass, easy to make stale
const [total, setTotal] = createSignal(0);
createEffect(() => setTotal(price() * qty()));

// ✓ memo: one tracked computation, read as total()
const total = createMemo(() => price() * qty());
```

## Pitfall — the infinite loop

An effect that **reads and writes the same signal** re-triggers itself forever:

```ts
createEffect(() => setCount(count() + 1));   // ✗ loops
```

Break the cycle: derive with a memo, use `untrack` for the read you don't want to
subscribe to, or make deps explicit with `on(source, () => ...)`.

## Pitfall — reading a dep outside the effect body

Only reads that happen **during** the effect's execution are tracked. A signal read
in a nested callback that runs later is not a dependency:

```ts
createEffect(() => {
  setTimeout(() => console.log(count()), 1000); // ✗ count() read later, untracked
});
```

If you need an explicit, stable dependency list, use `on`:

```ts
createEffect(on(count, (c) => console.log(c)));        // tracks count only
createEffect(on(count, (c) => ..., { defer: true }));  // skip the initial run
```

## Timing and cleanup

- `createEffect` runs **after** the DOM is updated. For pre-render work there's
  `createRenderEffect`; for layout reads, prefer it cautiously.
- `onMount` is just an effect that runs once — good for initial fetch / focus.
- `onCleanup` runs before every re-run *and* on disposal — use it for timers,
  listeners, subscriptions. Forgetting it is the usual source of leaks.

→ context7: `query-docs "createEffect on defer createRenderEffect onCleanup"`.
