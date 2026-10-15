# assets

Guards of Atlantis II images, copied from Stats of Atlantis
(github.com/QuasiStellar/Stats-of-Atlantis, `src/lib/images/`). `SOURCE`
holds the upstream commit and its date. The characters page shows
`avatars_full/` as each hero's banner, `cards/` on each card and
`spell_cards/` on Gydion's Spells tab; nothing else is used yet.

Copied by `scripts/fetch-assets.sh`. Run it again to refresh; it replaces
every folder here.

The artwork belongs to the game's publisher, Wolff Designa. The upstream
repository carries no licence file. Keep that in mind before publishing a
copy.

## Folders

| folder          | files | what                                                    |
| --------------- | ----- | ------------------------------------------------------- |
| `cards/<hero>/` | 610   | card illustrations, 775 x 1071 webp. Art only: no text, no numbers. |
| `avatars/`      | 32    | hero artwork, wide crop, 860 x 430 webp                  |
| `avatars_full/` | 32    | the same artwork, larger, 1456 x 720 webp                |
| `logos/`        | 32    | hero emblem, png                                         |
| `spell_cards/`  | 22    | Gydion's spellbook, 1192 x 831 webp. Whole card faces, text included. `Gold*` are the cantrips; `A` and `B` are a colour's two schools, `IIB2` the second school's other tier II spell. |
| `stat_icons/`   | 8     | white stat and role icons, png                           |
| `card_parts/`   | 155   | what upstream paints a card face from: frames, banners, action and item icons, tokens, markers, runes, role icons |

Hero folders and file names use the catalogue key ("wasp", "widget").

Upstream also holds images for two other games (`onward/`, `rivals_cards/`).
They are not copied.

## From a catalogue card to its illustration

Every card in `data/cards.json` has exactly one file in `cards/<hero>/`:

| card                         | file                                  |
| ---------------------------- | ------------------------------------- |
| Gold, Silver                 | `Gold.webp`, `Silver.webp`            |
| the handicap card            | `Handicap.webp`                       |
| the ultimate (purple)        | `Ultimate.webp`                       |
| an extra basic card          | `Extra1.webp`, `Extra2.webp`, in catalogue order (Takahide only) |
| red, blue, green, tier I-III | colour, tier, then `A` or `B`: `RedIIB.webp` |

`A` or `B` is the catalogue's `variant.first`: 1 is `A`, 2 is `B`. Tier I
cards have no variant and are always `A`. `web/src/cards/art.ts` applies
this rule.

## Icons in card text

Card text names icons as `::attack_red::`, `::token_zombie::`,
`::rune_axe::`. Each is `card_parts/<name>.png`, with one exception:
`::radius_silver::` is `card_parts/area_silver.png`.
