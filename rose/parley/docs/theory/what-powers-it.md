# What powers it — the machinery behind each feature

The skill map says _why_ each feature exists. This says _what actually drives it_:
which model, which provider, which capability. It is the bridge from pedagogy to
`src/providers/`.

## The design rule: one trait per capability

Every capability the app needs sits behind a Rust trait, and any unwired one falls
back to a stub so the app still boots. This is the load-bearing decision — it is
what lets "everything local" be a _choice_ rather than a constraint, and what lets
new features reuse capabilities instead of adding servers.

| Capability      | Trait          | Local default (in-process, Candle) | Alternative              |
| --------------- | -------------- | ---------------------------------- | ------------------------ |
| Conversation    | `Conversant`   | Qwen2.5-Instruct (GGUF, quantized) | Anthropic Claude (cloud) |
| Speech → text   | `Transcriber`  | Whisper                            | —                        |
| Text → speech   | `Synthesizer`  | Piper (macOS `say` as stopgap)     | —                        |
| Word → definition | `Dictionary` | Wiktionnaire (fr.wiktionary API)   | any source (TLFi, licensed API) — see [dictionary.md](../architecture/dictionary.md) |

The three voice capabilities compose into most of the skill pages; the
`Dictionary` is the one external lookup, feeding saved-word definitions. Widening
coverage stays cheap: most new features re-order existing traits rather than
adding one.

## Each feature as a composition

Read `→` as "feeds".

- **Converse** (live): `Transcriber` → `Conversant` → `Synthesizer`. The full
  voice loop. Audio in, French text, French reply, audio out.
- **Listen** (planned): `Conversant` (generate a short passage at i+1) →
  `Synthesizer` (speak it). Learner answers a comprehension check. **No new
  capability** — it drops ASR from the Converse loop and adds a question.
- **Read** (planned): `Conversant` only (generate a levelled passage; gloss words
  on tap). The cheapest feature to power — one capability, no audio.
- **Write** (planned): `Conversant` (react to the learner's written text with a
  recast). Text in, text out — the Converse loop minus both ASR and TTS, minus
  time pressure.
- **Vocabulary / SRS** (in progress): `Dictionary` (define a saved word) + the
  **learner-state** store (which words, and their definitions). No LLM: words are
  logged during Converse, defined by the Wiktionnaire on save, and — next — will be
  scheduled by a spaced-repetition function. See
  [dictionary.md](../architecture/dictionary.md).
- **Pronunciation / shadowing** (planned): `Synthesizer` (reference audio) +
  `Transcriber` (hear the learner's repeat) to compare. Reuses both audio traits.
- **Progress** (in progress): no model. Pure read of the learner-state store.

The pattern to notice: **most features add no new capability.** They recombine
`Conversant`, `Transcriber`, `Synthesizer`, the learner-state store, and the one
external `Dictionary`. This is the payoff of the trait-per-capability design and
the reason the build order in the skill map is realistic.

## Where the intelligence lives per turn

The conversational brain is stateless across turns _by itself_ — each turn we feed
it the whole history plus the pedagogy system prompt (`pedagogy.rs`). The
"learning" that persists between sessions does **not** live in the model; it lives
in the learner-state store. Keeping those two separate — a swappable brain, a
durable memory — is what lets us change models (Qwen ↔ Claude) without losing the
learner's history.
