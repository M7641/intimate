# Control flow: Show / For / Index / Switch

Solid's control-flow **components** exist because raw JS conditionals and `.map()`
don't track the way you'd hope, and they re-create DOM you wanted to keep.

**API recap (verify with context7 → `query-docs "For Show Index Switch"`)**

```tsx
<Show when={user()} fallback={<Login />}>{u => <Profile user={u()} />}</Show>
<For each={todos()}>{(todo, i) => <Todo item={todo} index={i()} />}</For>
<Switch fallback={<NotFound />}>
  <Match when={state() === "loading"}><Spinner /></Match>
  <Match when={state() === "ready"}><Data /></Match>
</Switch>
```

## `<For>` vs `.map()` — the keying difference

```tsx
{todos().map(t => <Todo item={t} />)}   // ✗ re-creates rows on most updates
<For each={todos()}>{t => <Todo item={t} />}</For>   // ✓ keyed by reference
```

`<For>` is **keyed by item reference**: it moves/keeps DOM nodes whose item object is
unchanged, and only creates/destroys what actually changed. `.map()` has no such
identity tracking, so updating one item can rebuild the whole list and lose focus,
scroll, and component state.

- **`<For>`** — list of *objects*; identity = the reference. Default choice.
- **`<Index>`** — key by *position*. Use when items are primitives, or when the slot
  matters more than the value (e.g. editable inputs at fixed positions). Here the
  item is a signal `item()` and the index is static.

Picking the wrong one is a real bug: `<For>` over primitives re-creates on every
edit; `<Index>` over reordered objects keeps stale DOM.

## `<Show>` vs ternary vs `&&`

```tsx
{user() && <Profile />}                 // works, but no fallback, and the read sits
                                        // in JSX — fine for trivial cases
<Show when={user()} fallback={<Login/>}>{u => <Profile user={u()} />}</Show>
```

Prefer `<Show>` when:
- you need a **fallback**,
- the block is non-trivial (Show creates/destroys it as one unit cleanly),
- you want the **callback form** `{u => ...}` which narrows `when` to a non-null
  value and only re-runs the children when `when` toggles, not on every change.

A ternary across large branches re-evaluates both sides' JSX expressions and can
thrash; `<Switch>/<Match>` is the clean multi-branch form.

→ context7: `query-docs "For Index keyed Show keyed Switch"` for callback-form
details and current props.
