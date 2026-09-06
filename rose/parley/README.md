# parley

A conversational language-learning companion. You _talk_ to it in the language
you are learning and it talks back — patiently, at your level, correcting gently
as you go. First target language: **French**.

## Why a conversation, not flashcards

> **Design docs.** The full theory — why parley has the features it has, and what
> powers each — lives in [`docs/`](docs/README.md): the [skill map](docs/theory/skill-map.md),
> [what powers it](docs/theory/what-powers-it.md), and the
> [learner-state spine](docs/architecture/learner-state.md).

The design follows the most robust findings in second-language acquisition (SLA).
Each principle maps to a concrete product decision:

1. **Comprehensible input (Krashen, "i+1").** We acquire language by understanding
   messages just above our current level. → The bot speaks one notch above the
   learner: not gibberish, not baby-talk.
2. **Pushed output (Swain).** Producing language forces the leap from meaning to
   form. → Spoken conversation is the core mechanic, not a gimmick.
3. **Negotiation of meaning (Long).** Progress spikes when a partner reacts to your
   errors mid-flow. → The bot does gentle _recasts_: it restates what you got wrong,
   correctly and naturally, without breaking the conversation.
4. **Spaced repetition.** Words stick when re-seen just before they fade. → The bot
   remembers words you struggled with and reintroduces them later. _(later milestone)_
5. **Low affective filter.** Anxiety blocks acquisition; a patient, judgement-free,
   always-available partner beats an intimidating tutor. → This is where a bot wins.
6. **Shadowing / prosody.** Repeating after a native speaker trains the ear and accent.
   → Replay-and-repeat the bot's audio. _(later milestone)_

## Everything is local and open — in-process, no servers

By default: no cloud, no API keys, and no separate model servers to run. Models
load straight into the Axum process via [Candle](https://github.com/huggingface/candle),
Hugging Face's Rust ML framework, and run on the Metal GPU. Weights download from
the HF Hub on first run and are cached.

The trait-per-capability design makes the local default a choice, not a
constraint: an optional Anthropic Claude provider is wired in behind the same
`Conversant` trait for when you want a stronger tutor with nothing to download
(see [Configuration](#configuration)).

- **LLM** (the conversational brain): Qwen2.5-Instruct (GGUF, quantized), in-process
  via Candle. Multilingual, ungated, fast. → `providers/candle_llm.rs`
  - _Optional cloud alternative:_ Anthropic Claude over the Messages API (raw
    HTTP, no SDK needed). Enabled by an env var; takes over the LLM slot when set.
    → `providers/anthropic.rs`
- **ASR** (speech → text): Whisper, in-process via Candle (Metal). The browser
  sends raw 16 kHz mono PCM, so the server decodes nothing. → `providers/candle_whisper.rs`
- **TTS** (text → speech): macOS `say` (built-in French voices) as a simple
  stopgap until a Rust-native HF TTS is wired. → `providers/say.rs` _(planned)_

Each capability sits behind a Rust trait, so an unwired one falls back to a stub
and the app still boots.

## Architecture

```
frontend (SolidJS)            backend (Axum) — single process, models in-memory
  hold-to-talk   ──audio──▶  POST /api/converse
  audio playback ◀──json───   1. Transcriber  (Candle Whisper)  audio → FR text
  transcript                  2. Conversant   (Candle Qwen2.5)  text  → FR reply
                              3. Synthesizer  (macOS say)       reply → audio
```

The conversation history plus the pedagogy system prompt are fed to the LLM each
turn. There is no separate inference server — Axum loads the weights and runs the
generation loop itself, serialized behind a mutex (one learner at a time).

### The backend is the app

The Rust crate is the project's front door: its binary is a [clap](https://docs.rs/clap)
CLI that both serves the API and drives the frontend, so the whole app runs
through one command. The SolidJS app is a nested `frontend/` it manages.

```
parley/
├── Cargo.toml        # the crate lives at the root — backend is the app
├── src/              # main.rs (CLI) · lib.rs (router) · providers/ · …
└── frontend/         # SolidJS + Vite, built and served by the backend
```

```
parley <cmd>
  serve    run the API (default with no subcommand; serves the built SPA too)
  build    npm ci + vite build  → frontend/dist
  dev      vite dev server only  (UI on :5173, proxying /api to the backend)
  start    vite + backend together for local dev, torn down as a unit
  install  npm install
```

In production `serve` also serves the built SPA from `frontend/dist`; in local
dev there's no bundle, so `serve` is API-only and Vite serves the UI.

## Where we are

The **conversational brain works end-to-end.** A quantized Qwen2.5-3B loads into
the Axum process via Candle (Metal) and holds a real French conversation. The
pedagogy lands as intended: replies stay short, pitch at the learner's level, end
with an open question, and correct errors by gentle recast rather than lecture.

Verified by hand:

- `je suis allé **à le** parc et j'ai mangé **un** pomme`
  → `Je suis allé **au** parc et j'ai mangé **une** pomme. Comment s'est passé votre journée ?`

Both errors fixed in passing, conversation kept flowing — that is principle 3
(negotiation of meaning) doing its job. `pedagogy.rs` carries this; two unit tests
guard it.

What's live vs stubbed:

| Capability        | Status                                    |
| ----------------- | ----------------------------------------- |
| Conversant (LLM)  | ✅ in-process Candle + Qwen2.5, or Anthropic Claude — both validated |
| Transcriber (ASR) | ✅ in-process Candle Whisper (small), validated on real French audio |
| Synthesizer (TTS) | ⬜ stub — macOS `say` next                |

Today you can talk to it: **speech in** (Candle Whisper) and **text reply out**
both work end-to-end. The voice loop is **one TTS provider from complete** — the
tutor's reply comes back as text; spoken audio is still the silent stub.

## Where we're heading

**Next — finish M1 (the voice loop).** One provider left, no architecture change:

1. ✅ **ASR: Candle Whisper, in-process.** Done. The browser captures the mic as
   16 kHz mono PCM, so the backend needs no ffmpeg and no audio decoding — "as raw
   as possible." → `providers/candle_whisper.rs`.
2. **TTS: macOS `say`.** Built-in French voices (Thomas/Amélie), zero dependencies,
   no server. A deliberate stopgap until a Rust-native HF TTS matures; swappable
   behind the `Synthesizer` trait.

Housekeeping: `ollama.rs` and `piper.rs` are leftovers from the pre-Candle design
and are no longer wired — remove when convenient. (The subprocess `whisper.rs` was
removed once Candle Whisper landed.)

**Then — the milestones that make it a tutor, not just a voice chatbot:**

- [ ] **M2 — Pedagogical memory.** Track estimated level + struggled-with words as
      an event log (the event-sourced shape used in `flow`). The conversation becomes a
      stream of learning events, not just chat history.
- [ ] **M3 — Spaced repetition.** Reintroduce weak vocabulary in later conversations,
      just before it would be forgotten.
- [ ] **M4 — Shadowing.** Replay-and-repeat the bot's audio to train ear and accent.

**Vocabulary page enrichment.** Saving a word already pulls its definition from a
trusted French dictionary (the Wiktionnaire — see
[`docs/architecture/dictionary.md`](docs/architecture/dictionary.md)). Next, we want
to **pull in example sentences** alongside each definition, so a saved word is shown
in real use, not just glossed. The Wiktionnaire is already the source: its wikitext
carries usage examples as `#*` lines under each sense (currently skipped), so this
is an extension of the existing lookup rather than a new source.

## Running

Everything runs through the backend binary from the project root. For local dev,
one command brings up Vite + the backend together (and tears them down as a unit):

```sh
cargo run -- start            # Vite on :5173, backend on :3000
cargo run -- start --install  # first time: install frontend deps too
```

`start` hot-reloads the backend via [bacon](https://github.com/Canop/bacon) if it's
installed (`cargo install bacon`), otherwise it runs once. Open the app at
http://localhost:5173 — Vite proxies `/api` to the backend, so the browser talks
same-origin. Prefer to run the pieces separately? `cargo run -- serve` and, in
another shell, `cargo run -- dev`.

For a production-style single server, build the SPA first — then `serve` hosts it:

```sh
cargo run -- build            # frontend/dist
cargo run --                  # serve (default): app at http://localhost:3000
```

On first `serve`, models download from the HF Hub (cached after): the local
Candle LLM (~1.9 GB, skipped when `ANTHROPIC_API_TOKEN_WORK` is set — Claude is
cloud) and the Whisper ASR model (~465 MB, skipped with `PARLEY_WHISPER_REPO=stub`).
Both load onto Metal before the server listens.

Quick text-only check (no audio):

```sh
curl -s localhost:3000/api/chat -H 'content-type: application/json' \
  -d '{"text":"Salut, ça va ?"}'
# → {"reply":"Salut ! Ça va ? Et toi, comment apprends-tu le français ?", ...}
```

### Configuration

The conversational brain is picked at startup, in this order (each choice is
logged, so it is obvious which one won):

1. **Anthropic Claude** — if `ANTHROPIC_API_TOKEN_WORK` is set. No weights to
   download; the tutor is strong out of the box. This is the opt-in cloud path.
2. **Candle in-process** — otherwise, the local default.
3. **Stub** — if Candle also fails to load (cold cache, bad file name). The app
   always boots.

To use Claude, export your API key (the model defaults to `claude-opus-4-8`):

```sh
export ANTHROPIC_API_TOKEN_WORK=sk-ant-...
export PARLEY_ANTHROPIC_MODEL=claude-opus-4-8   # optional override
```

The local Candle model is chosen by env vars (defaults shown). Bump to a stronger
model — e.g. `Qwen/Qwen2.5-7B-Instruct-GGUF` — by overriding the repo, file, and
tokenizer:

```sh
PARLEY_LLM_REPO=Qwen/Qwen2.5-3B-Instruct-GGUF
PARLEY_LLM_FILE=qwen2.5-3b-instruct-q4_k_m.gguf
PARLEY_LLM_TOKENIZER=Qwen/Qwen2.5-3B-Instruct
```

Speech-to-text runs Whisper in-process by default (`openai/whisper-small`,
~465 MB, downloaded once and cached). Override the model, or turn ASR off
entirely for offline dev and tests (falls back to the stub transcriber):

```sh
PARLEY_WHISPER_REPO=openai/whisper-small   # any 80-mel multilingual model
PARLEY_WHISPER_REPO=stub                   # or `off` — skip the download
```
