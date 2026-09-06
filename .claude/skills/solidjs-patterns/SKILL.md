---
name: solidjs-patterns
description: >-
  Diagnose and fix SolidJS apps that misbehave because of how Solid's fine-grained
  reactivity actually works — not how React-shaped intuition assumes it does. Use
  this whenever writing, reviewing, or debugging SolidJS components and you hit
  symptoms like "my signal doesn't update the UI", "the component renders once and
  never re-renders", "the value is stale", "props lost their reactivity after I
  destructured them", "createEffect runs at the wrong time / infinitely / not at
  all", "the store mutation didn't trigger anything", "why <For> instead of .map",
  "<Show> vs a ternary", "createResource / Suspense not updating", or any reactivity
  bug where data changes but the DOM doesn't (or vice-versa). Trigger even when the
  user just says "my Solid app is going badly", "fix this Solid component", "why
  isn't this reactive", "review my SolidJS code", or pastes a component that doesn't
  update. Carries the opinionated patterns and failure-mode → fix index; defers the
  live API surface to context7 (resolve-library-id "solidjs" → query-docs). SolidJS
  core only — not SolidStart routing/SSR. For React use the frontend skills.
---

# SolidJS patterns

Solid looks like React and is nothing like React. There is **no virtual DOM and no
re-render**. A component function runs **once**. After that, only the reactive
primitives you wired up re-run — and they re-run *surgically*, updating the exact
DOM node or computation that depends on a changed signal.

Almost every "my Solid app is going badly" bug is the same root cause: **a reactive
read got disconnected from its tracking scope**, so a change happens but nothing is
subscribed to hear it. This skill is an index of the ways that happens and the fix
for each.

## The one mental model that prevents most bugs

> A component body runs **once**. Reactivity lives only inside *tracking scopes*:
> JSX, `createEffect`, `createMemo`, and a few helpers. A signal read **outside** a
> tracking scope is a one-time snapshot — it will never update.

If you internalise only that sentence, you avoid the majority of Solid pitfalls.
The references below are the specific cases.

## Failure-mode index (symptom → cause → reference)

| Symptom you observe | Likely cause | Go to |
| --- | --- | --- |
| Prop value never updates after first render | Props were **destructured** → reactivity lost | `references/reactivity.md` |
| `const v = count()` and `v` is stale | Signal read into a plain var **outside** tracking scope | `references/reactivity.md` |
| Whole list re-creates / loses DOM state on update | Used `.map()` instead of `<For>` / wrong key choice | `references/control-flow.md` |
| `<Show>` vs ternary vs `&&` — which and why | Control-flow components track; raw JS often doesn't | `references/control-flow.md` |
| Store update does nothing | Mutated a copy, or wrong setter path / replaced the proxy | `references/state-stores.md` |
| Should I use a signal or a store? | Granularity & nesting decision | `references/state-stores.md` |
| `createEffect` loops forever / runs too early / never | Effect writes a dep it reads, or reads outside its body | `references/effects-lifecycle.md` |
| Derived value recomputes too much or is stale | Should be a `createMemo`, or deps read lazily | `references/effects-lifecycle.md` |
| Async data flashes, double-fetches, or never resolves | `createResource` / `Suspense` wiring | `references/data-async.md` |
| "It worked in React" and now it doesn't | You are fighting the no-re-render model | `references/anti-patterns.md` |

## How to use this skill

1. Match the symptom in the table above, open that one reference. Don't read all of
   them — each is self-contained.
2. For **exact current API** (signatures, new options, version changes) call
   context7: `resolve-library-id` with `solidjs`, then `query-docs`. The references
   here carry the *opinion and the before/after*, not the full API — that stays
   live so it can't go stale.
3. When reviewing code, scan for the anti-patterns in `references/anti-patterns.md`
   first — they catch the highest-frequency breakage in the fewest reads.
4. Need a correct starting shape rather than a fix? Copy from `templates/` — each
   file is a working exemplar that bakes in the patterns above:
   - `templates/Component.tsx` — props (no destructuring), `<Show>`/`<For>`, memo.
   - `templates/store.ts` — `createStore` with path setters, `produce`, `reconcile`.
   - `templates/resource.tsx` — `createResource` source-driven, `Suspense` + errors.
