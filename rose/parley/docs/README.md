# parley — design docs

This folder records **why parley has the features it has, and what powers them**.
It is the source of truth for the product's pedagogical thesis; the code reflects
it, not the other way round.

parley is not "a chatbot with extras". It is a bet that a language is best learned
by _doing the four things you actually do in a language_ — listen, speak, read,
write — with a patient partner, and that each of those is backed by a specific,
robust finding in second-language acquisition (SLA). Every feature here traces to
one of those findings.

## Read in this order

1. **[theory/skill-map.md](theory/skill-map.md)** — the map. The four skills, the
   three systems that cut across them, and the glue that turns pages into a
   journey. Each area is tied to the SLA principle it serves. Start here.
2. **[theory/what-powers-it.md](theory/what-powers-it.md)** — the machinery. Which
   model / provider / capability actually drives each feature, and why the
   trait-per-capability design lets the local-first default be a choice.
3. **[architecture/learner-state.md](architecture/learner-state.md)** — the glue,
   in detail. The learner-state layer is what makes "i+1" and spaced repetition
   possible; without it every page is an island. Its design and its current thin
   implementation.
4. **[architecture/dictionary.md](architecture/dictionary.md)** — saving a word and
   collecting its definition from a trusted French dictionary (the Wiktionnaire),
   behind a swappable `Dictionary` trait. Feeds the Vocabulary page.

## The one-line version

> A language is four skills (listen, speak, read, write) resting on three systems
> (vocabulary, grammar, prosody), held together by a model of what _this_ learner
> knows. parley builds the skills as pages, grows the systems _through_ them, and
> keeps the learner-state layer as the spine.
