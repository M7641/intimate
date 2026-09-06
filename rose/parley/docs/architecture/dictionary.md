# The dictionary — trusted definitions for saved words

The learner can **save a word**, and parley collects its **definition from a
trusted French dictionary**. This doc records why that dictionary, how the lookup
works, and where the definition then lives.

## Why saved words at all

parley already captures every word the learner produces (`vocab` memories, see
[learner-state.md](learner-state.md)). Saving is a layer of *deliberate curation*
on top: the learner marks a word worth keeping, and in return gets its meaning,
attributed to a real source. Saved words are the seed of the future
spaced-repetition schedule — the difference between "a word I once said" and "a
word I am choosing to learn".

So the feature is two moves:

1. **Save** — mark a word (typed, or one-tap from the words parley heard you say).
2. **Define** — fetch its senses from a trusted dictionary and keep them.

## The `Dictionary` capability

Definitions come from outside parley, so — like the three voice capabilities —
the source sits behind a trait in `providers/`:

```
trait Dictionary {
    async fn define(&self, word: &str) -> Result<Definition>;
}
```

A `Definition` is a small value: the word, the source name and URL (for
attribution and "read more"), and a list of `Sense`s, each a part of speech plus a
plain-text gloss. It lives in `domain.rs` because both the provider (produces it)
and memory (stores it) need it.

Handlers depend on `dyn Dictionary`, never on Wiktionary — which is what let the
source change mid-build without touching a route (see the gotcha below).

## Which dictionary — and the trade-off

**Chosen: the Wiktionnaire (fr.wiktionary.org), read through the MediaWiki
`action=parse` API.** It is the French-language Wiktionary — genuinely French
definitions, in keeping with parley's immersion thesis — is free and keyless, and
is openly licensed (CC BY-SA), so we can display and attribute it cleanly.

The honest trade-off, recorded so it can be revisited:

- The **authoritative** French dictionaries — the *Trésor de la Langue Française
  informatisé* (TLFi/CNRTL), *Le Robert*, *Larousse*, the *Dictionnaire de
  l'Académie* — have **no clean public API**. Using them would mean fragile HTML
  scraping or a paid licence. The Wiktionnaire is community-authored rather than
  academy-sanctioned, but it is broad, current, and machine-readable.
- Because the source sits behind the `Dictionary` trait, moving to the TLFi (or a
  licensed API) later is a one-file change. The trait is the insurance policy for
  this exact trade-off.

### The gotcha that shaped the implementation

The obvious route — Wikimedia's REST `page/definition/{word}` endpoint — **returns
`501 Internal error` on fr.wiktionary**. That endpoint is only enabled on the
*English* Wiktionary. Its French entries (`en.wiktionary.org/.../definition/chat`,
filtered to the `fr` section) do exist, but the glosses come back **in English** —
wrong for an immersion tutor.

So we skip the REST definition endpoint and instead fetch the page's **wikitext**
(`action=parse&prop=wikitext`) and extract the senses ourselves. The wikitext is
stable and deterministic to parse:

- a language section opens with `== {{langue|fr}} ==` (the next `== {{langue|…` ends
  it, so Old French / English sections on the same page are ignored);
- a part of speech opens with `=== {{S|nom|fr…}} ===` — the first argument is the POS;
- each sense is a line starting with `#` (excluding `#*` examples and `#:` quotes).

Each sense is then cleaned of wiki markup — templates `{{…}}`, links
`[[target|text]]`, and `''italics''` — down to plain French. All of this lives in
`providers/wiktionary.rs`, verified by unit tests on a fixture plus one `#[ignore]`
live test against the real API.

## Where the definition lives — fetch once, then offline

A fetched definition is stored **on the saved `VocabItem`** in the learner's
memories. Consequences:

- **Fetched once.** After saving, the definition is served from the memory store
  (local files or MinIO) — no repeat network calls, and it works offline.
- **Best-effort, non-fatal.** Saving is the learner's intent; the definition is
  enrichment. If the lookup fails (offline, or the word is not in the
  Wiktionnaire), the word is still saved with `definition: null`, and re-saving it
  later retries the lookup. A dictionary that is down never blocks saving.

## Endpoints

| Route              | Does                                                          |
| ------------------ | ------------------------------------------------------------ |
| `POST /api/words`  | Save `{ word }`: look up the definition, upsert the memory, return the item. |
| `GET /api/words`   | List saved words (with definitions), newest first — the Vocabulary page. |

Configuration: `PARLEY_WIKTIONARY_URL` overrides the source origin;
`PARLEY_DICTIONARY=stub` swaps in a canned definition for offline dev and tests.
