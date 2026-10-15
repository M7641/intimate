import { For, Show, createMemo, createSignal } from "solid-js";
import type { HeroSheet } from "../types";
import type { Tab } from "./CardsApp";
import { hasGuide } from "./guides";

type Props = {
  sheets: HeroSheet[];
  current: string | undefined;
  /// The open tab. A hero's link keeps it, where that hero has the tab.
  tab: Tab;
};

const STARS = [1, 2, 3, 4];

/// Heroes grouped by complexity. The filter matches name, set and roles.
export function HeroList(props: Props) {
  const [query, setQuery] = createSignal("");

  const shown = createMemo(() => {
    const q = query().trim().toLowerCase();
    if (!q) return props.sheets;
    return props.sheets.filter(({ profile: p }) =>
      [p.name, p.set, ...p.roles].some((field) => field.toLowerCase().includes(q)),
    );
  });

  return (
    <nav class="hero-list">
      <input
        type="search"
        placeholder="Filter by name, set or role"
        value={query()}
        onInput={(e) => setQuery(e.currentTarget.value)}
      />
      <For each={STARS}>
        {(stars) => {
          const group = () => shown().filter((s) => s.profile.stars === stars);
          return (
            <Show when={group().length}>
              <h2 title={`complexity ${stars} of 4`}>{"★".repeat(stars)}</h2>
              <For each={group()}>
                {({ profile: p }) => (
                  <a
                    href={`#${p.key}${props.tab === "guide" && hasGuide(p.key) ? "/guide" : ""}`}
                    classList={{ current: p.key === props.current }}
                  >
                    <span class="name">{p.name}</span>
                    <span class="set">{p.set}</span>
                  </a>
                )}
              </For>
            </Show>
          );
        }}
      </For>
      <Show when={!shown().length}>
        <p class="hint">No hero matches.</p>
      </Show>
    </nav>
  );
}
