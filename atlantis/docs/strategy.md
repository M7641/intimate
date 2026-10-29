# Strategy analysis

How we extract strategy knowledge about the game, and how we check it.

An LLM does not know this game. It does not need to. The knowledge is in the
repo: the rulebook in `rules/`, the community guides in `guides/`, the cards
in `../data/cards.json`. The LLM reads, structures and proposes. The engine
decides.

## The loop

> guides and MOBA genre → claim → bot policy → win rate → claim kept or dropped

1. Extract claims from the guides, each with a quote.
2. Group claims that several guides make independently. These are candidate
   general principles.
3. Turn a testable claim into a bot variant in `core/src/sim.rs`.
4. Play the variant against the plain bot, headless, a few hundred games.
5. Keep the claim if the numbers move. Record the result next to the claim.

## Sources of claims

**Guides.** The main source. One player's experience per hero, written down.

**The MOBA genre.** The LLM knows Dota and League well: wave management,
pushing and freezing a lane, trading, tempo, zoning, the value of a kill at
different moments. Mapped onto this game's battle zone, waves and initiative,
these give hypotheses. They are never facts until the engine confirms them.

**Simulated games.** Logs of bot games, read for why the losing team lost.
Patterns found there are new claims, and go back into the loop.

## The principles file

`principles.json` holds the claims. Each one has:

- `scope`: `general` if it holds across heroes, `hero` if it rests on one
  hero's cards.
- `evidence`: quotes from the guides, with the file they come from.
- `status`: `current` if it matches `cards.json`, `stale` if the card it
  relies on changed in the second printing, `unchecked` otherwise.
- `engine`: `testable-now`, `needs-card-text` (the engine does not apply card
  effects yet) or `needs-opponent-model` (the bots do not react to each
  other yet).
- `test`: the bot variant that would measure it, where one exists.

A claim backed by one guide stays a claim. It becomes a general principle
when a second guide makes it independently, or when the engine confirms it.

## Rules for extraction

- Quote the guide. A claim without a quote is not recorded.
- Check the printing. Most 2022 guides predate the second printing. Compare
  any card they name against `cards.json` before trusting the advice.
- Record disagreements between guides. They point at questions the engine can
  settle.
- Leave geometry to the engine. LLMs reason poorly about hex distance and
  line of sight.
- Ask for the case against a claim, not just for it. An LLM tends to agree
  with whatever it is given.

## Other uses of the LLM

**Rules checking.** Read a rules chapter, write its edge cases as engine
tests. This keeps the engine faithful to the rulebook, so the loop measures
the real game.

**Playing directly.** Possible: give the LLM the snapshot and the legal
actions. Slow, costly and tactically weak. Better used to write policies than
to pick moves.

## Status

Pilot run over four guides: Brogan, Tigerclaw, Rowenna, Swift. Eighteen
claims, seven of them backed by two guides or more. No claim has been tested
in the engine yet.
