// Reference store — fine-grained nested state done right.
//
// Demonstrates the patterns from references/state-stores.md:
//   - createStore for nested/structured state (not a signal-of-object)
//   - path-based setters, never direct mutation
//   - produce for ergonomic multi-step local edits
//   - reconcile to merge server data without losing node identity
//
// Pattern: build the store inside a factory so each caller gets its own instance,
// or call it once at module scope for a singleton. Confirm the update-grammar with
// context7: query-docs "createStore produce reconcile array update".

import { createStore, produce, reconcile } from "solid-js/store";

export type Todo = { id: string; title: string; done: boolean };
type TodoState = { filter: "all" | "active" | "done"; todos: Todo[] };

export function createTodoStore(initial: Todo[] = []) {
  const [state, setState] = createStore<TodoState>({
    filter: "all",
    todos: initial,
  });

  return {
    state, // read fine-grained: state.filter, state.todos[i].done — each tracks alone

    setFilter(filter: TodoState["filter"]) {
      // ✓ path setter — only readers of state.filter are notified
      setState("filter", filter);
    },

    add(todo: Todo) {
      // ✓ produce: mutable draft, still fine-grained under the hood
      setState(produce(s => {
        s.todos.push(todo);
      }));
    },

    toggle(id: string) {
      // ✓ target an array item by predicate, then set one leaf
      setState("todos", t => t.id === id, "done", done => !done);
    },

    rename(id: string, title: string) {
      setState("todos", t => t.id === id, "title", title);
    },

    remove(id: string) {
      setState("todos", todos => todos.filter(t => t.id !== id));
    },

    // ✓ replace from the server WITHOUT re-creating every node:
    //   reconcile diffs by key, so unchanged rows keep their identity (and DOM).
    syncFromServer(fresh: Todo[]) {
      setState("todos", reconcile(fresh, { key: "id" }));
    },
  };
}

// ✗ Anti-pattern, for contrast — do NOT do this:
//   const [s, set] = createStore({ todos: [] });
//   s.todos.push(todo);            // mutation on a read-only proxy → no update
//   set({ todos: [...s.todos] });  // replacing the whole tree → loses granularity
