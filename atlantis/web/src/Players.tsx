import { For, Show } from "solid-js";
import type { Action, Card, Player, Snapshot } from "./types";
import { CardRow } from "./CardRow";
import { TEAM_COLOR } from "./Board";

type Props = {
  snapshot: Snapshot;
  onAction: (a: Action) => void;
  onInspect: (c: Card) => void;
};

/// Every player's dashboard: level, coins, items, and cards by state.
/// In the Play phase a player's hand is clickable; in Defend, the
/// defender's; in LevelUp, the upgradable cards.
export function Players(props: Props) {
  const phase = () => props.snapshot.phase;

  const actionFor = (p: Player, c: Card): Action | undefined =>
    props.snapshot.actions.find(
      (a) =>
        a.player === p.id &&
        ((a.type === "PlayCard" && a.card === c.id) ||
          (a.type === "Defend" && a.card === c.id) ||
          (a.type === "Upgrade" && a.card_out === c.id)),
    );

  const isActor = (p: Player) => {
    const ph = phase();
    return (
      (ph.type === "Act" && ph.player === p.id) ||
      (ph.type === "Defend" && ph.defender === p.id) ||
      (ph.type === "LevelUp" && ph.player === p.id)
    );
  };

  return (
    <div class="players">
      <For each={props.snapshot.players}>
        {(p) => (
          <section class="player" classList={{ actor: isActor(p) }}>
            <header style={{ "border-color": TEAM_COLOR[p.team] }}>
              <b style={{ color: TEAM_COLOR[p.team] }}>{p.hero_name}</b>
              <span class="meta">
                lvl {p.level} · {p.coins} coin{p.coins === 1 ? "" : "s"}
                <Show when={p.items.length}>
                  {" · "}
                  {p.items.map(([i, n]) => `+${n} ${i.toLowerCase()}`).join(", ")}
                </Show>
                <Show when={p.ultimate}> · ultimate</Show>
              </span>
            </header>
            <Show when={p.played}>
              {(c) => <CardRow card={c()} label="played" onInspect={props.onInspect} />}
            </Show>
            <For each={p.hand}>
              {(c) => {
                const a = () => actionFor(p, c);
                return (
                  <CardRow
                    card={c}
                    clickable={a() !== undefined}
                    onClick={() => a() && props.onAction(a()!)}
                    onInspect={props.onInspect}
                  />
                );
              }}
            </For>
            <For each={p.resolved}>
              {(c) => <CardRow card={c} dim label="resolved" onInspect={props.onInspect} />}
            </For>
            <For each={p.discard}>
              {(c) => <CardRow card={c} dim label="discarded" onInspect={props.onInspect} />}
            </For>
          </section>
        )}
      </For>
    </div>
  );
}
