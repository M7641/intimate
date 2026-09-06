# genisis — the context database, and why an agent needs one

A companion to the [`README.md`](./README.md). The README explains _what_ `saga`
does and _how_ to run it. This document answers the two questions the pilot
raises:

1. **How** does a "context database for AI agents" really _enable_ an agent —
   what does it unlock that a bare LLM call cannot have?
2. **How** would a data/ML team use one — and when should you **not** add one?

The core (`embed`, `extract`, `store`, `retrieve`, `memory`) is ~670 lines, about
half of which are comments. It proves the mechanics. Everything here is about
turning that proof into leverage.

---

## Part 0 — The problem, stated cleanly

An LLM is a **pure function**: `answer = f(prompt)`. No state between calls.
Everything the agent "knows" at time T must fit in the prompt of call T. But the
prompt is:

- **bounded** — finite context window (and every token costs);
- **ephemeral** — nothing survives the call;
- **non-selective** — putting 10,000 past messages in is neither possible nor
  useful ("lost in the middle" even degrades quality).

A context database is the layer that turns `f(prompt)` into something that
_behaves_ like an agent with a memory:

```
        without memory                        with a context database
   ┌───────────────────┐              ┌───────────────────────────────────┐
   │  user ─► f(prompt) │              │  user ─► assemble_context() ─► f() │
   │           │        │              │              ▲          │         │
   │        answer      │              │          recall()    remember()   │
   │  (forgets all)     │              │              └── store ──┘         │
   └───────────────────┘              └───────────────────────────────────┘
```

The database does not "make the agent smarter" — it gives it **continuity,
selectivity, and freshness**. That is all, and it is enormous.

---

## Part 1 — How it enables an agent (the five levers)

Each lever maps to a part of the pilot, so you can tie the concept back to code.

### 1. Cross-session continuity — `store.facts` + `memory.assemble_context`

The agent recovers a user in a session that shares **no** chat history with the
previous one. This is the sharp difference between a _context database_ and a
plain conversation buffer: semantic memory is indexed by **entity**, not by
**conversation**. Without it, every session starts from scratch — the agent asks
your name again every time.

> In the demo: section 2. The agent has no messages on friday yet re-injects the
> facts learned on monday.

### 2. Selectivity under budget — `retrieve.retrieve` (hybrid RRF)

The window is small; we cannot re-inject everything. Retrieval picks the subset
_relevant to the current turn_. Hybrid because no single source is enough:

| Source             | Strong at                                        | Blind to                                 |
| ------------------ | ------------------------------------------------ | ---------------------------------------- |
| Vector (cosine)    | fuzzy semantic similarity ("lives" ~ "based in") | the exact, rare entities (a name, a SKU) |
| Keyword (FTS/BM25) | the exact, rare identifiers                      | synonyms, paraphrase                     |
| Recency            | breaking ties at equal relevance                 | relevance itself                         |

We fuse them via **Reciprocal Rank Fusion** (`score = Σ 1/(k₀+rank)`). RRF
ignores score scale (cosine ∈ [−1,1] vs BM25 ∈ ℝ⁺), so no fragile calibration —
which is exactly why production uses it.

### 3. Freshness guaranteed by construction — bi-temporality (`store`)

When a fact replaces another ("I moved"), we do **not** `UPDATE`. We **close**
the old one (`valid_to = tick`) and insert the new one. Retrieval by default only
reads facts where `valid_to IS NULL`. Consequence: a stale fact _cannot_ surface
into the context — freshness is a property of the schema, not a `WHERE` a dev can
forget.

> In the demo: sections 3–4. "Manchester" becomes stale; no query brings it back;
> only "Lisbon" is served.

### 4. Auditability / reproducibility — time-travel (`facts_asof`)

"Why did the agent answer _that_ on monday?" becomes a query
`recall(q, asof=monday_tick)`, not log archaeology. For a production agent
(support, finance, healthcare), being able to **replay the exact memory state**
of a past decision is not a luxury — it is a compliance requirement.

> In the demo: section 5. The same query returns Manchester at monday's tick,
> Lisbon now.

### 5. Decoupling — `memory.Memory` (the 3-function API)

The agent stays _stateless_ and only knows `remember` / `recall` /
`assemble_context`. All persistence (embedding, extraction, storage, fusion) sits
behind that interface. You can change the vector store, or move extraction from
regex to an LLM, **without touching the agent**. That is what you buy by treating
memory as a _database_ and not as code scattered across the prompt.

---

## Part 2 — How _we_ (a data/ML team) use it

Two angles: **building** agents on top, and **operating** the database as a data
asset.

### Decision — when to reach for this pattern

| Signal                                                                                      | Why a context database helps                                                                                |
| ------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| The agent must **remember across sessions** (preferences, profile, past decisions).         | This is lever 1. A conversation buffer is not enough: you need an index by entity.                          |
| Facts **change and contradict** over time (a ticket's status, a model version, an address). | Bi-temporality (lever 3) keeps the history _and_ always serves the current. An `UPDATE` destroys the audit. |
| You exceed what **fits in the window** and "re-inject everything" degrades quality.         | Selective retrieval (lever 2) bounds the context to what is relevant.                                       |
| You need to **explain/replay** an agent decision (compliance, debugging).                   | Time-travel (lever 4) makes past memory state queryable.                                                    |
| Several agents/tools share a **common user profile**.                                       | The database becomes the shared source of truth, not N diverging copies in N prompts.                       |

### When _not_ to add one

- **Stateless task** (classify a document, translate, summarise a PDF). No
  continuity to maintain → no memory. Don't build a bridge you won't cross.
- **A single short session** that fits entirely in the window. The conversation
  buffer _is_ already your memory; a database is pure complexity.
- **Static, shared knowledge** (product docs, codebase, policies). That is
  **RAG**, not memory — see the box below. The two coexist but are not the same.
- **The LLM-extraction cost is not justified.** Consolidating every message with
  an LLM call is the most expensive line in the pipeline. If facts are rare or
  rarely reused, store the raw episodes and retrieve over them directly.

> **RAG vs memory — the distinction that avoids confusion.** RAG retrieves from an
> _external, shared, mostly static_ corpus (the docs). Memory retrieves from an
> _internal, per-agent/per-user, evolving_ state that the agent wrote itself. Same
> retrieval primitive (vector+FTS), opposite purposes: RAG answers "what do the
> docs say?", memory answers "what do I know about _you_ / what happened?". `saga`
> is a memory. A real system often has both, behind the same `assemble_context`.

### Operating the database as a data asset (work we already know)

This is where a data/ML team has an edge: a context database **is** a data
problem, not a prompt problem.

- **Extraction quality = data quality.** Extraction (the seam `extract.py`) is a
  classifier/extractor. Fact precision/recall, eval sets, drift detection: exactly
  our trade. A bad extractor poisons the memory the way bad ingestion poisons a
  model.
- **Embeddings = feature engineering.** The encoder choice (seam `embed.py`), its
  dimension, its fine-tuning on our domain: that is feature work, measurable
  (recall@k on labelled queries).
- **Governance.** Bi-temporality = lineage and the right to be forgotten _by
  construction_ (closing and purging a fact past a retention window is trivial;
  tracing all sources of a fact via `source_episode` is too).
- **Evaluation.** "Does memory serve the right fact?" is an offline retrieval
  metric (recall@k, MRR) _plus_ an end-to-end metric (does the agent answer better
  with the memory?). We know how to build both.

---

## Part 3 — Adjacent work (what this pilot lacks, and the order to add it)

The pilot proves the core. Here are the threads to pull, from most directly
useful to most speculative.

### Tier 1 — Harden the core

1. **Plug in a real encoder** (`embed.py`). _(small)_ — Replace hashing with
   BGE / nomic / OpenAI. This is the only seam whose limit is _visible_ in the
   demo ("located" ≠ "location"). Measuring recall@k before/after on a labelled
   query set proves the gain.
2. **LLM extraction** (`extract.py`). _(small–medium)_ — A structured call
   ("return the facts as JSON"). Unlocks the phrasings regexes miss _and_ entity
   resolution (beyond `subject="user"`). It is the most expensive line; putting it
   behind a cache/batch is a real topic.
3. **Entity resolution + graph.** _(medium)_ — Today everything is "user". A real
   system (Zep/Graphiti) builds a _graph_ of bi-temporal entities and relations:
   "Alex → works_at → Nimbus → based_in → Manchester". Retrieval then follows edges,
   not just text similarity.

### Tier 2 — Memory behaviours

4. **Decay & forgetting.** _(medium)_ — Not all memories age equally. Weight
   retrieval by a decay (recency × access frequency × confidence), and
   purge/summarise cold facts. Without it the database bloats and the signal
   dilutes.
5. **Hierarchical consolidation / summarisation.** _(medium)_ — Compress N
   episodes into a summary ("working memory" → "episodic" → "semantic"),
   MemGPT-style. Lets you keep a long horizon under a fixed context budget.
6. **Finer contradiction reconciliation.** _(medium)_ — Here contradiction is
   binary (same single-valued key → close). A real system handles confidence,
   competing sources, and "partial invalidation" (a fact stays true in one
   context, not another).

### Tier 3 — Product / platform

7. **Multi-tenant + isolation.** _(medium)_ — One memory per user/agent, with
   strict isolation at the storage level (not a `WHERE tenant_id` you forget — the
   same argument as row-level security in `fusion`).
8. **Memory service (API).** _(medium)_ — Expose `remember`/`recall` behind an
   endpoint so several agents/languages share the database. This is the product
   shape of OpenViking/Letta/Zep.
9. **Memory eval harness.** _(medium)_ — A reproducible bench: conversation sets
   → expected facts → queries → expected served facts. Measures extraction _and_
   retrieval end to end. This is what turns memory from "works in a demo" to "we
   know when it regresses".

---

## Suggested order of attack

```
1. Real encoder              (Tier 1.1)   measurable gain, unblocks retrieval
2. LLM extraction            (Tier 1.2)   covers real language + entity resolution
3. Eval harness              (Tier 3.9)   so you don't regress blindly afterwards
4. Decay / forgetting        (Tier 2.4)   keeps the signal clean as it grows
5. Entity graph              (Tier 1.3)   the "memory" → "structured knowledge" jump
```

Each step is a runnable artefact that de-risks the next. The core
(bi-temporality, RRF, episodic/semantic) does not move — we only swap seams and
enrich the behaviours around it.

---

## Reading

- **MemGPT / Letta** (Packer et al., 2023) — the agent that manages its own
  hierarchical memory under a context budget. The source of the working/episodic
  pattern.
- **Zep / Graphiti** — agent memory as a _bi-temporal knowledge graph_. The direct
  generalisation of this pilot's `facts` table.
- **Mem0** — a pragmatic memory layer (LLM extraction + vector store); a good
  reference for the `extract.py` seam in production.
- **Reciprocal Rank Fusion** (Cormack et al., 2009) — why fuse ranks rather than
  scores. The heart of `retrieve.py`.
- **"Lost in the Middle"** (Liu et al., 2023) — empirical proof that "re-inject
  everything" hurts; the justification for selective retrieval.
- **Bi-temporal modeling** (Snodgrass, _Developing Time-Oriented Database
  Applications_) — `valid_from`/`valid_to`, the theoretical basis of lever 3.
