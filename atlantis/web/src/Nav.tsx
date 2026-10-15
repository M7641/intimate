import { For } from "solid-js";

// One entry per HTML page (see vite.config.ts). Plain links: each page
// loads on its own and shares no state with the others.
const PAGES = [
  { href: "/", label: "Game" },
  { href: "/cards.html", label: "Characters" },
];

export function Nav() {
  const here = location.pathname === "/index.html" ? "/" : location.pathname;
  return (
    <nav class="pages">
      <For each={PAGES}>
        {(p) => (
          <a href={p.href} classList={{ current: p.href === here }}>
            {p.label}
          </a>
        )}
      </For>
    </nav>
  );
}
