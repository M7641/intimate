import { Show } from "solid-js";
import type { Card } from "./types";

export const COLOR_HEX: Record<Card["color"], string> = {
  GOLD: "#d4a017",
  SILVER: "#9aa3ad",
  RED: "#c0392b",
  GREEN: "#3d8b3d",
  BLUE: "#2c6fbd",
  PURPLE: "#7b3fa0",
};

const TIER = ["", "I", "II", "III", "IV"];

export function primaryLabel(c: Card): string {
  if (!c.primaryAction) return "ultimate";
  const kind = c.primaryAction.replace("DEFENSE_SKILL", "DEF/SKILL").toLowerCase();
  const v = c.primaryValue !== null ? ` ${c.primaryValue}` : "";
  const sign = c.primaryValueSign === "PLUS" ? "+" : c.primaryValueSign === "EXCLAMATION" ? "!" : "";
  const mod = c.modifier ? ` ${c.modifier.toLowerCase()} ${c.modifierValue ?? ""}` : "";
  return `${kind}${v}${sign}${mod}`;
}

type Props = {
  card: Card;
  clickable?: boolean;
  dim?: boolean;
  label?: string;
  onClick?: () => void;
  onInspect?: (c: Card) => void;
};

/// One compact line per card. Click to act on it, hover the name to inspect.
export function CardRow(props: Props) {
  return (
    <div
      class="card-row"
      classList={{ clickable: props.clickable, dim: props.dim }}
      style={{ "border-left-color": COLOR_HEX[props.card.color] }}
      onClick={() => props.clickable && props.onClick?.()}
      onMouseEnter={() => props.onInspect?.(props.card)}
    >
      <span class="init">{props.card.initiative || "–"}</span>
      <span class="name">
        {props.card.name}
        <Show when={props.card.tier}>
          <sup>{TIER[props.card.tier!]}</sup>
        </Show>
      </span>
      <span class="stats">
        {primaryLabel(props.card)}
        <Show when={props.card.secondaryAttack}> · atk {props.card.secondaryAttack}</Show>
        <Show when={props.card.secondaryMovement}> · mv {props.card.secondaryMovement}</Show>
        <Show when={props.card.secondaryDefense}> · def {props.card.secondaryDefense}</Show>
      </span>
      <Show when={props.label}>
        <span class="tag">{props.label}</span>
      </Show>
    </div>
  );
}
