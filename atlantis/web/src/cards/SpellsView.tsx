import { For } from "solid-js";
import type { Color } from "../types";
import { COLOR_HEX } from "../CardRow";

// Only Gydion casts from a spellbook. Hard-wired, and shown as scans, until
// the engine serves the spells themselves (data/spells.json).
export const hasSpells = (hero: string) => hero === "gydion";

// Spell card faces from assets/, as one URL per file.
const SPELL_ART = import.meta.glob<string>("../../../assets/spell_cards/*.webp", {
  eager: true,
  query: "?url",
  import: "default",
});
const art = (name: string) => SPELL_ART[`../../../assets/spell_cards/${name}.webp`];

const CANTRIPS = ["GoldI", "GoldII", "GoldIII"];
const COLORS: Color[] = ["RED", "BLUE", "GREEN"];
// Down a column: a colour's first school by tier, then its second school,
// which has two tier II spells.
const SLOTS = ["IA", "IIA", "IIIA", "IIB", "IIB2", "IIIB"];

const title = (color: Color) => color[0] + color.slice(1).toLowerCase();

function Spell(props: { name: string; label: string }) {
  return <img src={art(props.name)} alt={props.label} loading="lazy" />;
}

/// Gydion's spellbook: the three cantrips, each colour's two schools, Wish.
export function SpellsView() {
  return (
    <div class="spellbook">
      <h3>Cantrips</h3>
      <div class="spells">
        <For each={CANTRIPS}>{(name) => <Spell name={name} label="Cantrip spell card" />}</For>
      </div>

      <h3>Schools</h3>
      <div class="spells">
        <For each={COLORS}>
          {(color) => (
            <div class="column-head" style={{ "border-color": COLOR_HEX[color] }}>
              {color.toLowerCase()}
            </div>
          )}
        </For>
      </div>
      <div class="spells schools">
        <For each={COLORS}>
          {(color) => (
            <For each={SLOTS}>
              {(slot) => <Spell name={`${title(color)}${slot}`} label={`${title(color)} spell card, ${slot}`} />}
            </For>
          )}
        </For>
      </div>

      <h3>Ultimate</h3>
      <div class="spells">
        <Spell name="Ultimate" label="Ultimate spell card" />
      </div>
    </div>
  );
}
