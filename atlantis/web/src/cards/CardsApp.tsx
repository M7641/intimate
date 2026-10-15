import { Show, createMemo, createResource, createSignal, onCleanup } from "solid-js";
import { loadEngine } from "../engine";
import { Nav } from "../Nav";
import { HeroList } from "./HeroList";
import { HeroSheetView } from "./HeroSheetView";

export type Tab = "cards" | "guide" | "spells";

/// "#wasp" is Wasp's cards, "#wasp/guide" her guide.
function parseHash(): { key: string; tab: Tab } {
  const [key, tab] = location.hash.slice(1).split("/");
  return { key, tab: tab === "guide" || tab === "spells" ? tab : "cards" };
}

/// The character browser: pick a hero on the left, read the deck or the
/// guide on the right. Hero and tab live in the URL hash, so a link or the
/// back button finds them.
export function CardsApp() {
  const [sheets] = createResource(async () => (await loadEngine()).catalog());

  const [route, setRoute] = createSignal(parseHash());
  const onHash = () => setRoute(parseHash());
  window.addEventListener("hashchange", onHash);
  onCleanup(() => window.removeEventListener("hashchange", onHash));

  // Kept here, above the sheet, so it holds from one hero to the next.
  const [handicap, setHandicap] = createSignal(false);

  const current = createMemo(() => {
    const all = sheets();
    return all?.find((s) => s.profile.key === route().key) ?? all?.[0];
  });

  return (
    <div class="app">
      <header>
        <h1>Atlantis</h1>
        <Nav />
        <Show when={sheets.error}>
          <span class="error">engine failed to load: {String(sheets.error)}</span>
        </Show>
        <Show when={sheets()}>
          {(all) => (
            <span class="status">
              {all().length} heroes · {all().reduce((n, s) => n + s.cards.length, 0)} cards
            </span>
          )}
        </Show>
      </header>
      <Show when={sheets()} fallback={<p class="hint">Loading the catalogue.</p>}>
        {(all) => (
          <main>
            <HeroList sheets={all()} current={current()?.profile.key} tab={route().tab} />
            <Show when={current()} keyed>
              {(sheet) => (
                <HeroSheetView sheet={sheet} tab={route().tab} handicap={handicap()} onHandicap={setHandicap} />
              )}
            </Show>
          </main>
        )}
      </Show>
    </div>
  );
}
