import { For, Show } from "solid-js";
import type { Card, Color, HeroSheet, Rating } from "../types";
import { COLOR_HEX } from "../CardRow";
import { CardTile } from "./CardTile";
import { deckArt } from "./art";
import type { Tab } from "./CardsApp";
import { GuideView } from "./GuideView";
import { hasGuide } from "./guides";
import { SpellsView, hasSpells } from "./SpellsView";

const SCALE = [1, 2, 3, 4, 5, 6, 7, 8];

// Hero artwork from assets/, as one URL per file. Vite copies a file into
// the build only because it is named here; the browser fetches it on use.
const ARTWORK = import.meta.glob<string>("../../../assets/avatars_full/*.webp", {
  eager: true,
  query: "?url",
  import: "default",
});
const artwork = (key: string) => ARTWORK[`../../../assets/avatars_full/${key}.webp`];

// The upgrade grid's columns, left to right.
const UPGRADE_COLORS: Color[] = ["RED", "BLUE", "GREEN"];
const COLUMNS = UPGRADE_COLORS.length;

/// One stat as eight pips: filled up to the base, outlined up to the peak.
function RatingBar(props: { label: string; rating: Rating }) {
  const peak = () => props.rating.peak ?? props.rating.base;
  return (
    <div class="rating">
      <span class="label">{props.label}</span>
      <span class="pips">
        <For each={SCALE}>
          {(n) => <i classList={{ base: n <= props.rating.base, peak: n > props.rating.base && n <= peak() }} />}
        </For>
      </span>
      <span class="value">
        {props.rating.base}
        <Show when={props.rating.peak}> ({props.rating.peak})</Show>
      </span>
    </div>
  );
}

type Props = {
  sheet: HeroSheet;
  tab: Tab;
  /// Show the handicap card in place of the basic card it weakens.
  handicap: boolean;
  onHandicap: (on: boolean) => void;
};

/// A hero's profile and whole deck on one grid, so every card is the same
/// size: basic cards first, then a column per colour and a row per upgrade.
export function HeroSheetView(props: Props) {
  const profile = () => props.sheet.profile;
  const cards = () => props.sheet.cards;
  const art = () => deckArt(profile().key, cards());
  const guide = () => hasGuide(profile().key);
  const spells = () => hasSpells(profile().key);
  // A tab this hero lacks falls back to the cards.
  const tab = (): Tab =>
    (props.tab === "guide" && guide()) || (props.tab === "spells" && spells()) ? props.tab : "cards";

  // Gold, Silver, ultimate: one row. The handicap card takes the place of
  // the card of its colour (the Gold, for most heroes) when switched on.
  // Extra basic cards, if the hero has any, follow on the next row.
  const basics = (): Card[] => {
    const basic = cards().filter((c) => c.tier === null);
    const handicap = basic.find((c) => c.handicapped);
    const slot = (color: Color) =>
      props.handicap && handicap?.color === color
        ? handicap
        : basic.find((c) => c.color === color && !c.handicapped && !c.extra)!;
    return [
      slot("GOLD"),
      slot("SILVER"),
      ...cards().filter((c) => c.color === "PURPLE"),
      ...basic.filter((c) => c.extra),
    ];
  };
  // Every upgrade with its cell: the column is its colour, and down the
  // column run tier I, the two tier II cards, then the two tier III cards.
  const upgrades = () =>
    UPGRADE_COLORS.flatMap((color, column) =>
      cards()
        .filter((c) => c.color === color)
        .sort((a, b) => a.tier! - b.tier! || (a.variant?.first ?? 0) - (b.variant?.first ?? 0))
        .map((card, row) => ({ card, col: column + 1, row: row + 1 })),
    ).sort((a, b) => a.row - b.row || a.col - b.col);

  return (
    <div class="sheet">
      <div class="banner">
        <Show when={artwork(profile().key)}>{(src) => <img src={src()} alt="" />}</Show>
        <div class="title">
          <h2>{profile().name}</h2>
          <p class="meta">
            <span title={`complexity ${profile().stars} of 4`}>{"★".repeat(profile().stars)}</span> ·{" "}
            {profile().set} set
          </p>
          <p class="roles">
            <For each={profile().roles}>{(r) => <span class="role">{r}</span>}</For>
          </p>
        </div>
      </div>
      <div class="ratings">
        <RatingBar label="Attack" rating={profile().attack} />
        <RatingBar label="Initiative" rating={profile().initiative} />
        <RatingBar label="Defense" rating={profile().defense} />
        <RatingBar label="Movement" rating={profile().movement} />
      </div>

      <nav class="tabs">
        <a href={`#${profile().key}`} classList={{ current: tab() === "cards" }}>
          Cards
        </a>
        <Show when={guide()} fallback={<span title="No guide is linked for this hero.">Guide</span>}>
          <a href={`#${profile().key}/guide`} classList={{ current: tab() === "guide" }}>
            Guide
          </a>
        </Show>
        <Show when={spells()}>
          <a href={`#${profile().key}/spells`} classList={{ current: tab() === "spells" }}>
            Spells
          </a>
        </Show>
      </nav>

      <Show when={tab() === "guide"}>
        <GuideView hero={profile().key} />
      </Show>
      <Show when={tab() === "spells"}>
        <SpellsView />
      </Show>
      <div class="deck" hidden={tab() !== "cards"} style={{ "--basic-rows": Math.ceil(basics().length / COLUMNS) }}>
        <div class="section-head">
          <h3>Basic cards and ultimate</h3>
          <label>
            <input
              type="checkbox"
              checked={props.handicap}
              onChange={(e) => props.onHandicap(e.currentTarget.checked)}
            />
            Handicap card
          </label>
        </div>
        <For each={basics()}>{(c) => <CardTile card={c} art={art().get(c.id)} />}</For>
        <h3>Upgrades</h3>
        <For each={UPGRADE_COLORS}>
          {(color, column) => (
            <div
              class="column-head placed"
              style={{ "border-color": COLOR_HEX[color], "--col": column() + 1, "--row": 0 }}
            >
              {color.toLowerCase()}
            </div>
          )}
        </For>
        <For each={upgrades()}>
          {(u) => <CardTile card={u.card} art={art().get(u.card.id)} cell={{ col: u.col, row: u.row }} />}
        </For>
      </div>
    </div>
  );
}
