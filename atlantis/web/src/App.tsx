import { Show, createEffect, createMemo, createResource, createSignal, onCleanup } from "solid-js";
import { loadEngine, type Policy } from "./engine";
import { Board, TEAM_COLOR, type Mark } from "./Board";
import { Players } from "./Players";
import { Controls } from "./Controls";
import { Log } from "./Log";
import { CardRow } from "./CardRow";
import { Nav } from "./Nav";
import type { Action, Card, Hex, Snapshot } from "./types";
import { key } from "./hexmath";

export function App() {
  const [engine] = createResource(() => loadEngine());
  const [snapshot, setSnapshot] = createSignal<Snapshot | null>(null);
  const [selected, setSelected] = createSignal<Hex | null>(null);
  const [inspected, setInspected] = createSignal<Card | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [auto, setAuto] = createSignal(false);
  const [seed, setSeed] = createSignal(1);
  const policy: Policy = "brawler";

  const guard = (f: () => Snapshot) => {
    try {
      setSnapshot(f());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const newGame = () => {
    setAuto(false);
    setSelected(null);
    guard(() => engine()!.newGame(seed()));
  };
  const act = (a: Action) => guard(() => engine()!.apply(a));
  const step = () => guard(() => engine()!.step(policy));

  // Start a game as soon as the engine is in. `?steps=N` lets the bot open.
  createEffect(() => {
    if (!engine()) return;
    newGame();
    const n = Number(new URLSearchParams(location.search).get("steps") ?? 0);
    for (let i = 0; i < n; i++) step();
  });

  // Auto-play: one bot action every tick until the game ends.
  createEffect(() => {
    if (!auto()) return;
    const id = setInterval(() => {
      if (snapshot()?.phase.type === "GameOver") setAuto(false);
      else step();
    }, 150);
    onCleanup(() => clearInterval(id));
  });

  // Hexes that a click would act on, from the legal actions.
  const marks = createMemo(() => {
    const m = new Map<string, Mark>();
    const s = snapshot();
    if (!s) return m;
    const hexOfUnit = (id: number) => s.units.find((u) => u.id === id)?.hex ?? null;
    for (const a of s.actions) {
      if (a.type === "Move") m.set(key(a.to), "move");
      else if (a.type === "FastTravel") m.set(key(a.to), "travel");
      else if (a.type === "Respawn") m.set(key(a.hex), "respawn");
      else if (a.type === "Attack") {
        const h = hexOfUnit(a.target);
        if (h) m.set(key(h), "attack");
      }
    }
    return m;
  });

  const activeHero = () => {
    const s = snapshot();
    if (!s) return null;
    const ph = s.phase;
    const id = ph.type === "Act" ? ph.player : ph.type === "Defend" ? ph.defender : null;
    return id === null ? null : s.players[id].hero;
  };

  // A click on a marked hex applies the action; elsewhere it selects.
  const onHex = (hex: Hex) => {
    const s = snapshot();
    if (!s) return;
    const k = key(hex);
    const unitHere = s.units.find((u) => u.hex && key(u.hex) === k);
    const hit = s.actions.find(
      (a) =>
        ((a.type === "Move" || a.type === "FastTravel") && key(a.to) === k) ||
        (a.type === "Respawn" && key(a.hex) === k) ||
        (a.type === "Attack" && unitHere && a.target === unitHere.id),
    );
    if (hit) act(hit);
    else setSelected(hex);
  };

  const selectedInfo = () => {
    const s = snapshot();
    const h = selected();
    if (!s || !h) return null;
    const space = s.spaces.find((sp) => key(sp.hex) === key(h));
    if (!space) return null;
    const unit = s.units.find((u) => u.hex && key(u.hex) === key(h));
    const zone = s.zones.find((z) => z.id === space.zone)!;
    const spawn = space.spawn
      ? space.spawn.type === "Hero"
        ? `${space.spawn.team} hero spawn`
        : `${space.spawn.team} ${space.spawn.kind.toLowerCase()} spawn`
      : "";
    const who = unit
      ? unit.kind.type === "Hero"
        ? unit.kind.name
        : `${unit.team} ${unit.kind.kind.toLowerCase()} minion`
      : "";
    return [`${zone.name} (${h.q},${h.r})`, space.terrain ? "terrain" : "", spawn, who]
      .filter(Boolean)
      .join(" · ");
  };

  return (
    <div class="app">
      <header>
        <h1>Atlantis</h1>
        <Nav />
        <Show when={engine.error}>
          <span class="error">engine failed to load: {String(engine.error)}</span>
        </Show>
        <Show when={engine()}>
          <label>
            seed <input type="number" value={seed()} onInput={(e) => setSeed(Number(e.currentTarget.value))} />
          </label>
          <button onClick={newGame}>New game</button>
          <button onClick={step} disabled={snapshot()?.phase.type === "GameOver"}>Bot step</button>
          <button onClick={() => setAuto(!auto())} disabled={snapshot()?.phase.type === "GameOver"}>
            {auto() ? "Stop" : "Auto-play"}
          </button>
        </Show>
        <Show when={snapshot()}>
          {(s) => (
            <span class="status">
              round {s().round} · turn {s().turn} · waves {s().waves} ·{" "}
              <span style={{ color: TEAM_COLOR.Red }}>red lives {s().lives.red}</span> ·{" "}
              <span style={{ color: TEAM_COLOR.Blue }}>blue lives {s().lives.blue}</span> · coin{" "}
              {s().tie_breaker} · battle zone {s().zones.find((z) => z.id === s().battle_zone)?.name}
            </span>
          )}
        </Show>
        <Show when={error()}>
          <span class="error">{error()}</span>
        </Show>
      </header>
      <Show when={snapshot()} fallback={<p class="hint">Loading the engine.</p>}>
        {(snap) => (
          <main>
            <div class="center">
              <Controls snapshot={snap()} onAction={act} />
              <Board
                snapshot={snap()}
                selected={selected()}
                marks={marks()}
                activeHero={activeHero()}
                onSelect={onHex}
              />
              <div class="footer">
                <span class="selected-info">{selectedInfo() ?? "Click a hex to inspect it."}</span>
              </div>
            </div>
            <aside>
              <Players snapshot={snap()} onAction={act} onInspect={setInspected} />
              <Show when={inspected()}>
                {(c) => (
                  <section class="inspect">
                    <CardRow card={c()} />
                    <p class="text">{c().text}</p>
                  </section>
                )}
              </Show>
              <Log lines={snap().log} />
            </aside>
          </main>
        )}
      </Show>
    </div>
  );
}
