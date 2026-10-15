import { For, Show, createMemo } from "solid-js";
import type { Action, Snapshot } from "./types";
import { primaryLabel } from "./CardRow";

type Props = {
  snapshot: Snapshot;
  onAction: (a: Action) => void;
};

/// The current decision, in words, with buttons for the actions that are
/// not clicks on the board or on a card.
export function Controls(props: Props) {
  const s = () => props.snapshot;
  const name = (id: number) => s().players[id].hero_name;
  const cardOf = (playerId: number, cardId: number) => {
    const p = s().players[playerId];
    return [...p.hand, ...p.deck, ...(p.played ? [p.played] : [])].find((c) => c.id === cardId)!;
  };

  const title = createMemo(() => {
    const ph = s().phase;
    switch (ph.type) {
      case "Play":
        return "Everyone plays a card facedown. Click a card in a hand.";
      case "ChooseActor":
        return `Initiative tie: ${ph.candidates.map(name).join(" or ")} acts first?`;
      case "Act":
        return `${name(ph.player)} acts. Click a lit hex to move, a ringed unit to attack, or a button.`;
      case "Defend":
        return `${name(ph.defender)} is attacked for ${ph.attack_total}. Discard a card from hand to defend, or take the hit.`;
      case "LevelUp":
        return `${name(ph.player)} levels up. Pick the card to upgrade and its replacement.`;
      case "GameOver":
        return `${ph.winner} wins: ${ph.reason}.`;
    }
  });

  const buttons = createMemo(() =>
    s().actions.filter((a) =>
      ["Pass", "ChooseActor", "Skill", "Hold", "Upgrade"].includes(a.type) ||
      (a.type === "Defend" && a.card === null),
    ),
  );

  const label = (a: Action): string => {
    switch (a.type) {
      case "Pass":
        return `${name(a.player)} passes`;
      case "ChooseActor":
        return name(a.player);
      case "Skill":
        return `Skill: ${s().players[a.player].played?.name ?? ""}`;
      case "Hold":
        return "Hold";
      case "Defend":
        return "Take the hit";
      case "Upgrade": {
        const out = cardOf(a.player, a.card_out);
        const take = cardOf(a.player, a.take);
        return `${out.name} → ${take.name} (${primaryLabel(take)}, item ${take.item?.toLowerCase()})`;
      }
      default:
        return a.type;
    }
  };

  return (
    <section class="controls" classList={{ over: s().phase.type === "GameOver" }}>
      <p class="title">{title()}</p>
      <Show when={buttons().length}>
        <div class="buttons">
          <For each={buttons()}>
            {(a) => <button onClick={() => props.onAction(a)}>{label(a)}</button>}
          </For>
        </div>
      </Show>
    </section>
  );
}
