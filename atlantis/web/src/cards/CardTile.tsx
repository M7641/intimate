import { For, Show } from "solid-js";
import type { Card } from "../types";
import { COLOR_HEX } from "../CardRow";
import { RulesText } from "./RulesText";

const TIER = ["", "I", "II", "III", "IV"];

const ACTION: Record<NonNullable<Card["primaryAction"]>, string> = {
  ATTACK: "Attack",
  SKILL: "Skill",
  DEFENSE: "Defense",
  MOVEMENT: "Movement",
  DEFENSE_SKILL: "Defense / Skill",
};

const SIGN = { PLUS: "+", MINUS: "−", EXCLAMATION: "!" };

const lower = (s: string) => s.toLowerCase().replaceAll("_", " ");
const title = (s: string) => s[0] + lower(s).slice(1);

/// "Attack 4+", "Skill", "Defense !".
function primary(c: Card): string {
  if (!c.primaryAction) return "";
  const sign = c.primaryValueSign ? SIGN[c.primaryValueSign] : "";
  return [ACTION[c.primaryAction], `${c.primaryValue ?? ""}${sign}`].filter(Boolean).join(" ");
}

/// "Range 2", "Radius 3+". The catalogue calls a radius an area.
function reach(c: Card): string {
  if (!c.modifier) return "";
  const sign = c.modifierValueSign ? SIGN[c.modifierValueSign] : "";
  return `${c.modifier === "AREA" ? "Radius" : "Range"} ${c.modifierValue ?? ""}${sign}`;
}

function secondary(c: Card): string {
  return [
    c.secondaryAttack !== null && `Attack ${c.secondaryAttack}`,
    c.secondaryDefense !== null && `Defense ${c.secondaryDefense}`,
    c.secondaryMovement !== null && `Movement ${c.secondaryMovement}`,
  ]
    .filter(Boolean)
    .join(" · ");
}

function tag(c: Card): string {
  if (c.handicapped) return "handicap";
  if (c.extra) return "starts in deck";
  if (c.color === "PURPLE") return "ultimate";
  return "";
}

type Props = {
  card: Card;
  art?: string;
  /// Where the tile sits in the deck grid. Without it, the grid decides.
  cell?: { col: number; row: number };
};

/// One card in full: illustration, numbers, text and the catalogue's tags.
export function CardTile(props: Props) {
  const c = () => props.card;
  return (
    <article
      class="card-tile"
      classList={{ placed: props.cell !== undefined }}
      style={{ "border-left-color": COLOR_HEX[c().color], "--col": props.cell?.col, "--row": props.cell?.row }}
    >
      <Show when={props.art} fallback={<div class="art" />}>
        <img class="art" src={props.art} alt="" loading="lazy" />
      </Show>
      <div class="body">
        <div class="head">
          <Show when={c().primaryAction}>
            <span class="init" title="initiative">
              {c().initiative}
            </span>
          </Show>
          <span class="name">
            {c().name}
            <Show when={c().tier}>
              <sup>{TIER[c().tier!]}</sup>
            </Show>
          </span>
          <Show when={tag(c())}>
            <span class="tag">{tag(c())}</span>
          </Show>
        </div>
        <dl>
          <Show when={primary(c())}>
            <dt>Primary</dt>
            <dd>{[primary(c()), reach(c())].filter(Boolean).join(" · ")}</dd>
          </Show>
          <Show when={secondary(c())}>
            <dt>Secondary</dt>
            <dd>{secondary(c())}</dd>
          </Show>
          <Show when={c().item}>
            <dt>Item</dt>
            <dd>{c().item === "AREA" ? "Radius" : title(c().item!)}</dd>
          </Show>
        </dl>
        <RulesText text={c().text} />
        <Show when={c().traits.length}>
          <p class="traits">
            <For each={c().traits}>{(t) => <span>{lower(t)}</span>}</For>
          </p>
        </Show>
      </div>
    </article>
  );
}
