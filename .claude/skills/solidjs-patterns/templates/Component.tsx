// Reference component — the correct shape for a Solid component.
//
// Demonstrates the patterns from references/reactivity.md and control-flow.md:
//   - props are NEVER destructured (the proxy stays intact)
//   - mergeProps for reactive defaults, splitProps to extract + forward
//   - signal reads happen inside JSX / handlers, never snapshotted in the body
//   - createMemo for a derived value (not an effect)
//   - <Show> with the callback form, <For> keyed by reference
//
// Copy and adapt. Confirm exact signatures with context7 if in doubt:
//   resolve-library-id "solidjs" → query-docs "createSignal createMemo splitProps".

import { createMemo, createSignal, For, Show, splitProps, mergeProps } from "solid-js";
import type { JSX } from "solid-js";

type Todo = { id: string; title: string; done: boolean };

export type TodoListProps = {
  title?: string;                 // optional → gets a reactive default below
  todos: Todo[];                  // reactive source from the parent
  onToggle: (id: string) => void;
  // anything else (class, style, data-*) is forwarded untouched
} & JSX.HTMLAttributes<HTMLDivElement>;

export function TodoList(props: TodoListProps) {
  // ✓ reactive default — `merged.title` updates if the parent's prop changes
  const merged = mergeProps({ title: "Todos" }, props);

  // ✓ split our own props from the ones we forward to the wrapper element.
  //   `local` stays reactive; `rest` is spread onto the <div>.
  const [local, rest] = splitProps(merged, ["title", "todos", "onToggle"]);

  // ✓ local UI state as a signal
  const [hideDone, setHideDone] = createSignal(false);

  // ✓ derived value as a MEMO, not an effect — read it like a signal: visible()
  const visible = createMemo(() =>
    hideDone() ? local.todos.filter(t => !t.done) : local.todos,
  );

  return (
    <div {...rest}>
      <header>
        {/* ✓ read inside JSX — tracked */}
        <h2>{local.title}</h2>
        <label>
          <input
            type="checkbox"
            checked={hideDone()}
            // ✓ pass a function; use the updater form
            onChange={() => setHideDone(v => !v)}
          />
          Hide done
        </label>
      </header>

      {/* ✓ <Show> with callback form + fallback */}
      <Show when={visible().length > 0} fallback={<p>Nothing to show.</p>}>
        <ul>
          {/* ✓ <For> keyed by item reference — preserves DOM across updates */}
          <For each={visible()}>
            {(todo) => (
              <li>
                <label>
                  <input
                    type="checkbox"
                    checked={todo.done}
                    onChange={() => local.onToggle(todo.id)}
                  />
                  {todo.title}
                </label>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}
