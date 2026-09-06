# The skill map — why parley has the features it has

This is the pedagogical backbone. It answers one question: **what must an app do
to help someone genuinely learn a language, and why those things and not others?**

We refuse the "pile of features" approach. Instead we start from how linguists
actually decompose language competence, and let the pages fall out of that.

## Three layers

A language is not one thing you learn. It is:

1. **Four skills** — the things you _do_ in a language.
2. **Three systems** — the machinery every skill draws on.
3. **One spine** — a model of what _this_ learner knows, which makes the rest
   adaptive instead of generic.

Get the layering wrong and you get flashcards: isolated drills that feel like
progress but don't transfer to real use. parley's README rejects that explicitly;
this map is how we stay honest to it.

## Layer 1 — the four skills

Every skill is a pairing of a **channel** (spoken / written) and a **mode**
(understanding / producing). All four are needed; each is its own page because
each trains a genuinely different muscle.

| Page          | Skill               | Channel | Mode      | SLA principle it serves                     | Status |
| ------------- | ------------------- | ------- | --------- | ------------------------------------------- | ------ |
| **Converse**  | Speaking            | Spoken  | Producing | Pushed output (Swain) + negotiation (Long)  | Live   |
| **Listen**    | Listening           | Spoken  | Receiving | Comprehensible input (Krashen, "i+1")       | Planned |
| **Read**      | Reading             | Written | Receiving | Comprehensible input, at the learner's pace | Planned |
| **Write**     | Writing             | Written | Producing | Pushed output, without real-time pressure   | Planned |

Why this matters: parley today is excellent at exactly **one** of the four cells —
spoken production. That is deliberately the hardest and highest-value one to start
with, but it is one leg of a four-legged stool. Listening and reading are mostly
_reuse_ of machinery we already have (see what-powers-it.md), so they are the
cheapest way to widen coverage.

### Why "i+1" runs through all of it

Krashen's comprehensible-input hypothesis says we acquire language by
understanding messages pitched _just_ above our current level — "i+1". This is
already the dial in `pedagogy.rs::level_guidance`. But a dial is only as good as
its input: "one notch above the learner" is meaningless unless we know where the
learner _is_. That requirement is what forces Layer 3.

## Layer 2 — the three systems (NOT their own pages)

Skills draw on three cross-cutting systems. The tempting mistake is to give each
its own drill page — and land straight back in flashcards. Instead, each system
is a **view derived from what happens in the skills.**

- **Vocabulary → spaced repetition.** Not a deck of random words: the words _you_
  struggled with in conversation, resurfaced just before they fade (README
  milestone 4). The page exists to make that memory visible and to schedule
  review — the words come from real use, not a wordlist. _Partly built:_ you can
  save a word and get its definition from a trusted French dictionary (the
  Wiktionnaire); the spaced-repetition schedule is the next step. See
  [../architecture/dictionary.md](../architecture/dictionary.md).
- **Grammar → a journal of your recasts.** Not a page of rules. When the tutor
  gently recasts your error (Long's negotiation of meaning), we log it. The page
  shows those corrections grouped by pattern ("agreement", "à/au prepositions"),
  so grammar emerges from your own mistakes rather than being front-loaded.
- **Prosody → shadowing.** Replay the tutor's audio and repeat it, training ear
  and accent (README milestone 6). It rides on the TTS we already produce every
  turn.

The through-line: **systems are outputs of the skills, surfaced back to the
learner.** They are the reason the learner-state spine has to exist.

## Layer 3 — the spine (learner-state)

One model, updated every turn, of what this learner knows: estimated level, words
seen and struggled with, recast patterns, time-on-task. It is what:

- sets the "i" in "i+1" so input can be pitched (Layer 1),
- feeds spaced repetition and the grammar journal (Layer 2),
- powers the **Progress** page, the only page whose job is to reflect the learner
  back to themselves.

Detailed design: [../architecture/learner-state.md](../architecture/learner-state.md).

## Build order (and its justification)

We do **not** build eight shallow pages. Order by _respect for the thesis_ and
_reuse of existing machinery_:

1. **Learner-state spine + Progress page** — the glue first, or every later page
   is an island. Unlocks i+1 and spaced repetition.
2. **Vocabulary (SRS driven by conversation misses)** — strongest evidence base,
   milestone already planned.
3. **Listen** — reuses TTS/LLM, fills the most conspicuous missing cell
   (spoken reception).
4. **Read**, then **Write**, then **Pronunciation / shadowing.**

Each row above is one honest step, in the spirit of "the next clear step over
clever compression".
