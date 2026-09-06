# Anti-patterns: React habits that break Solid

Scan this first when reviewing a Solid component. These are the highest-frequency
breakages, almost all from assuming a re-render that never happens.

| React habit | Why it breaks in Solid | Do instead |
| --- | --- | --- |
| Destructure props: `function C({ x })` | Evaluates the getter once; reactivity lost | Keep `props.x`; `splitProps` / `mergeProps` to extract — see `reactivity.md` |
| `const v = signal()` in the body | Body runs once; `v` is frozen | Read `signal()` *inside* JSX / effect — see `reactivity.md` |
| Conditional early-return on a signal | Body runs once, so the branch is fixed | Use `<Show>` / `<Switch>` — see `control-flow.md` |
| `items.map(...)` for lists | No identity tracking; rebuilds DOM | `<For>` (objects) / `<Index>` (primitives) — see `control-flow.md` |
| Mutate store: `state.x = 1` | Proxy is read-only; no notification | `setState(...)` / `produce` — see `state-stores.md` |
| `useMemo`-style effect writing a signal | Extra render pass, stale risk | `createMemo` — see `effects-lifecycle.md` |
| Fetch inside `createEffect` | Races, double-fetch | `createResource` with a source — see `data-async.md` |
| Expect the component to "re-render" | There is no re-render in Solid | Wire the reactive primitive; only it re-runs |
| `key` prop on a list element | Not how Solid keys | `<For>` keys by reference; `<Index>` by position |

## The single diagnostic question

When a Solid component "doesn't update", ask:

> **Is the reactive read happening inside a tracking scope (JSX, `createEffect`,
> `createMemo`), or did it get snapshotted into a plain variable / destructured
> first?**

Nine times out of ten the read escaped its tracking scope. Trace the value back from
the DOM to its signal and find the line where it stopped being a function call.

## What to confirm against live docs

This file is opinion and pattern. For exact, current API — signatures, new options,
version-specific behaviour, SolidStart specifics (out of scope here) — use context7:
`resolve-library-id "solidjs"` then `query-docs` with the primitive name. Treat the
live docs as the source of truth for *syntax*; treat this skill as the source of
truth for *which pattern to reach for*.
