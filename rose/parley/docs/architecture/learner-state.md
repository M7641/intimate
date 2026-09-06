# The learner-state spine — memories about the learner

This is the glue described in the skill map's Layer 3. Everything adaptive in
parley — pitching input at "i+1", spaced repetition, steering the learner into
new territory — needs a durable answer to one question: **what does _this_ learner
know, and what have they not yet touched?**

That answer is the learner's _memories_: a small, growing record of who they are as
a language learner. This doc covers what we store, how it steers the tutor, and
where it lives (local files by default, MinIO when you want it).

## Why memories, not just chat history

Chat history is per-session and lives in RAM (`state.rs::Sessions`). It vanishes on
restart and knows nothing across conversations. Memories are the opposite: **few,
durable, cross-session facts** distilled from many conversations.

The distinction mirrors how a good human tutor works. They don't replay every past
sentence; they remember _"you're comfortable with the present tense, you keep
tripping on à/au, and we've never talked about work."_ That compressed profile is
what lets them pitch the next lesson. parley's memories are that profile, made
explicit.

## What a memory is

A memory is one small fact about the learner, in one of a few kinds:

| Kind        | Example                                             | Feeds                          |
| ----------- | --------------------------------------------------- | ------------------------------ |
| `level`     | estimated CEFR band, per skill                      | the "i" in i+1 (`pedagogy.rs`) |
| `topic`     | "talked about: food, family, travel"                | steering into new areas        |
| `interest`  | "likes cycling, works in healthcare"                | making input personally relevant |
| `struggle`  | "à vs au; gender of nouns ending -e"                | spaced repetition, grammar journal |
| `vocab`     | word seen, times struggled, last-seen timestamp     | spaced repetition schedule     |

Each is a small JSON object with a stable id, a kind, a payload, and a timestamp —
deliberately close to the shape of an event log so history is append-friendly.

## How memories push the learner into new areas

This is the mechanic the product hangs on. Left alone, a learner (and a chatty
LLM) circles the same comfortable topics — you end up fluent in ordering coffee and
lost everywhere else. Memories break that loop:

1. Every turn, we read the learner's `topic` memories and compute the **gap** —
   everyday domains they have _not_ practiced (work, health, hobbies, opinions,
   the past, the future…).
2. We inject a compact line into the pedagogy system prompt:
   _"The learner has practiced: food, family. Gently steer the conversation toward
   an area they have not: work, health, or plans for the future."_
3. The tutor, still following its recast/i+1 rules, naturally opens a door into
   new vocabulary — without the learner having to pick a lesson.

So the LLM does the steering, but the **memories decide the direction.** This keeps
the conversation-first thesis intact (no menus, no drills) while guaranteeing
breadth over time. Breadth of lexical domain is exactly what separates a
conversational beginner from a functional speaker.

## Where memories live — the `MemoryStore` trait

Persistence sits behind a trait, exactly like the three voice capabilities. The
handler depends on `dyn MemoryStore`, never on a concrete backend, so swapping
storage never touches the routes.

```
trait MemoryStore {
    async fn load(&self, learner: LearnerId) -> Result<Memories>;
    async fn save(&self, learner: LearnerId, memories: &Memories) -> Result<()>;
}
```

Two backends, selected from the environment just like Whisper/Piper:

- **`FsMemoryStore` (default).** One JSON file per learner under a local data dir.
  Zero servers, zero config — it honours parley's local-first thesis. Chosen when
  no S3 endpoint is configured.
- **`S3MemoryStore` (MinIO).** One object per learner in an S3 bucket. Chosen when
  `PARLEY_S3_ENDPOINT` is set. See below.

The default-to-stub-or-local pattern means the app still boots with no storage
configured (memories simply don't persist across restarts), and turns durable the
moment a backend is wired — the same graceful-degradation rule the providers follow.

## MinIO / S3 backend

[MinIO](https://min.io) is a single-binary, S3-compatible object store. Using the
S3 API (not a bespoke format) means the same code talks to MinIO locally today and
to AWS S3 or any S3-compatible store later, changing only credentials and endpoint.

Why object storage for this, rather than a database:

- Memories are **small, per-learner blobs** read whole and written whole — the
  natural grain of an object store, no schema or migrations.
- It matches the monorepo's existing habit of S3-backed data with S3Mock in tests,
  so integration tests can spin a disposable MinIO/S3Mock container and throw it
  away (see the Rust testing standards).
- It scales from "one file on my laptop" to "a shared bucket for many learners"
  without a code change.

Configuration (all env, mirroring the provider selection in `state.rs`):

| Variable              | Meaning                                    |
| --------------------- | ------------------------------------------ |
| `PARLEY_S3_ENDPOINT`  | MinIO URL, e.g. `http://localhost:9000`. Presence flips storage from FS to S3. |
| `PARLEY_S3_BUCKET`    | bucket name, e.g. `parley-memories`.       |
| `PARLEY_S3_REGION`    | region label (MinIO ignores it; default `us-east-1`). |
| `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` | credentials (MinIO defaults: `minioadmin`/`minioadmin`). |

Run MinIO locally:

```
docker run -p 9000:9000 -p 9001:9001 \
  -e MINIO_ROOT_USER=minioadmin -e MINIO_ROOT_PASSWORD=minioadmin \
  minio/minio server /data --console-address ":9001"
```

Path-style addressing is forced (MinIO needs it), and the bucket is created on
first use if absent.

## Current state vs target

- **Now (landed):** the `MemoryStore` trait with an `FsMemoryStore` default and an
  `S3MemoryStore` (MinIO) selected by env (`state.rs::select_memory_store`). Each
  conversation turn captures the learner's content words into `vocab` memories
  (`areas::content_words`, deterministic — no model) and persists them. Those words
  drive the **area-gap steering** (`areas::steer_line`), injected into the pedagogy
  prompt so the tutor opens doors into unpracticed everyday domains. A
  `/api/progress` endpoint reads it all back for the Progress page. The learner can
  also **save** a word, which attaches its definition (from the Wiktionnaire) to the
  `vocab` memory — see [dictionary.md](dictionary.md).
- **Next:** richer memory kinds beyond `vocab` — `topic`/`interest`/`struggle`
  distilled by the model (not just keyword-matched), a real spaced-repetition
  schedule over `vocab` for the Vocabulary page, and per-skill level estimation.

The spine already spans read-write (words captured shape the next turn's steer);
what remains is deepening _what_ we remember, one honest step at a time.
