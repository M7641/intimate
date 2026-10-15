import type { Card } from "../types";

// Card illustrations from assets/, as one URL per file. Vite copies them
// into the build; the browser fetches one only when a tile shows it.
const CARD_ART = import.meta.glob<string>("../../../assets/cards/*/*.webp", {
  eager: true,
  query: "?url",
  import: "default",
});

const TIER = ["", "I", "II", "III"];

/// The file name of a card's illustration. assets/README.md has the rule.
/// `extra` counts the extra basic cards met so far, this one included.
function artName(card: Card, extra: number): string {
  if (card.color === "PURPLE") return "Ultimate";
  if (card.handicapped) return "Handicap";
  if (card.extra) return `Extra${extra}`;
  const color = card.color[0] + card.color.slice(1).toLowerCase();
  if (card.tier === null) return color;
  return `${color}${TIER[card.tier]}${card.variant?.first === 2 ? "B" : "A"}`;
}

/// Illustration URL for every card of one hero's deck, by card id.
export function deckArt(hero: string, deck: Card[]): Map<number, string> {
  const art = new Map<number, string>();
  let extra = 0;
  for (const card of deck) {
    if (card.extra) extra += 1;
    const url = CARD_ART[`../../../assets/cards/${hero}/${artName(card, extra)}.webp`];
    if (url) art.set(card.id, url);
  }
  return art;
}
