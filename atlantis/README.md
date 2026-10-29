# atlantis

A simulator for the board game Guards of Atlantis II.

The rules engine is pure Rust. A browser page shows its state at every step,
so the engine and the view grow together. The same engine runs headless:
bot against bot, hundreds of games, statistics.

## Layout

| path       | what                                                        |
| ---------- | ----------------------------------------------------------- |
| `core/`    | the engine. Board, cards, players, turn machine, combat, end of round, bots. No I/O. |
| `wasm/`    | thin ABI exposing `core` to the browser. No logic.          |
| `web/`     | SolidJS pages: the game (SVG board, dashboards, log) and the characters page. Know no rules. |
| `data/`    | board layout, hero profiles, card catalogue.                |
| `assets/`  | card art, portraits and icons, copied from Stats of Atlantis. The characters page shows the hero and card artwork. |
| `docs/`    | `rules/`: the rulebook, transcribed. `guides/`: community hero guides. `strategy.md`: how strategy claims are extracted and tested; `principles.json`: the claims. |
| `scripts/` | `build-wasm.sh`: compile the engine to `web/public/`. `fetch-assets.sh`, `guides-from-bgg/`: refresh `assets/` and `docs/guides/`. |

## Run

```bash
proto install                    # repo root, once: rust, bun, moon, ...
moon run atlantis-web:dev        # builds wasm, starts Vite on :5180
```

`rust-toolchain.toml` in this directory adds the wasm32 target; rustup
installs it the first time cargo runs here. By hand:

```bash
cd atlantis
cargo test --workspace
./scripts/build-wasm.sh
cd web && bun install && bun run dev       # http://localhost:5180
```

Headless statistics:

```bash
cargo run -p atlantis-core --release --example stats -- 500
```

## The pages

Two HTML pages, linked from the header: the game at `/` and the characters
at `/cards.html`. Each loads on its own; they share the engine, not state.

### Game

The header shows round, turn, wave counters, life counters, the tie-breaker
coin and the battle zone. The yellow bar says whose decision it is.

- Play phase: click a card in any hand.
- Act phase: lit hexes are moves (green), fast travel (blue dashed) or
  respawn points (yellow). A red ring marks an attack target. Skill and Hold
  are buttons.
- Defend phase: click a card in the defender's hand, or "Take the hit".
- Level up: one button per upgrade choice.

"Bot step" lets the brawler bot take one action. "Auto-play" runs it to the
end. `?steps=40` in the URL opens the page after 40 bot actions.

Hover a card to read its text.

### Characters

Pick a hero on the left; the filter matches name, set and role. The sheet
opens on the hero's artwork, with the roles and stat profile, then two tabs.

The Guide tab shows the hero's community guide from `docs/guides/`, where one
exists. `#wasp/guide` in the URL opens on it.

Gydion has a third tab, Spells: the scans of his spellbook cards.

The Cards tab shows the whole deck on one grid of equal-size cards: Gold,
Silver and ultimate first, then a column per colour and a row per upgrade (I,
II first, II second, III first, III second). The "Handicap card" box shows the
handicap card in place of the basic card it weakens. Each card shows its
illustration, initiative, primary and secondary values, item icon, text, and
the catalogue's tags. `#wasp` in the URL opens on that hero.

## How the pieces talk

The engine exposes three game calls: new game, apply an action, bot step. Each
returns a snapshot: board, units, players with every card by state, the
phase, and the list of legal actions. The page builds its clicks and buttons
from that list and never decides anything itself.

The wasm crate writes each result as JSON into one buffer; `web/src/engine.ts`
reads it back. A panic lands in the same buffer, so the page shows it.

A fourth call, `catalog`, needs no game. It returns every hero's profile and
deck, read from the same data the engine plays with; the characters page renders
that and nothing else.

`web/src/types.ts` mirrors `core/src/game.rs`, `core/src/turn.rs` and
`core/src/hero.rs` by hand. Change one, change the other.

## Data

`data/board.ron` draws the map as text, one cell per hex, sea included.
Legend at the top of `core/src/board.rs`. It was transcribed from a top-down
photo of the printed board: the hex lattice was fitted to the photo, each hex
classified by colour, and the spawn icons read by hand against the rulebook's
icon key. The printed grid runs 15 degrees off the board's edges, so the page
rotates it back. Zone borders between the three sand zones may be off by a
hex here and there: edit the text and reload the page to correct one.

`data/cards.json` is the card catalogue from Stats of Atlantis
(github.com/QuasiStellar/Stats-of-Atlantis, `src/new_heroes.json`, the new
printing): 32 heroes, values and text.
`data/heroes.csv` is the hero profile sheet: complexity, roles, stats.
`data/spells.json` is Gydion's spellbook, from the same source
(`src/spells.json`): 22 spells in the catalogue's card format, plus a
`school`. The engine does not load it yet.

## What the engine applies

- Rounds of four turns. Simultaneous facedown play, reveal, initiative order,
  tie-breaker coin, same-team ties as a choice.
- One action per card: move, fast travel, attack, skill, hold, respawn.
  Obstacles, zones, fast-travel restrictions.
- Attack totals with items, defense by discard with items and minion
  modifiers, heavy minion immunity, minion bounties.
- Hero defeat: life counters by lowest tier, defeat and assist coins,
  unresolved card resolved.
- End of round: minion battle, lane push (mid-turn too), respawn in the new
  battle zone, card retrieval, mandatory level ups with the two-card upgrade
  choice and items, ultimate at level 8, pity coin.
- Three ways to win: throne captured, last push, no life counters left.

## What it does not apply yet

- Card text. Every card's numbers work; its text is shown, not executed.
  Skills therefore do nothing. Block defenses are not offered.
- Tokens, markers, active effects ("This turn:"), pushes, swaps. All of
  these come with card text.
- Minion battle removals are chosen by the engine (melee first, heavy last),
  not by the losing team.
- Hidden information: the page shows every hand.

## Status

- [x] hex grid, zones, spawn points, zone adjacency
- [x] units on the board: teaching game line-up, minions on their spawn points
- [x] movement and fast travel
- [x] cards, hands, initiative, tie-breaker
- [x] attack and defense
- [x] end of round: minion battle, lane push, level up
- [x] bots and headless statistics
- [ ] card text: an effect language for the teaching heroes' 72 cards
- [x] board transcribed from a top-down photo: zones, rocks, every spawn point
