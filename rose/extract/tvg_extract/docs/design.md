# Design — how `extract` works and why it is shaped this way

This document covers the theory: how the module turns a generative model into a
structured extractor, how the modules compose, and why the package offers
several independent paths rather than one. For how to run it, see the
[README](../README.md); for directions not yet built, see
[`directions.md`](directions.md).

## How extraction works (the LLM path)

Turn a **generative** model into a **structured extractor** by constraining both
ends: a precise prompt in, strict validation out.

1. **Prompt synthesis.** `ExtractionSchema.prompt_block()` renders the field
   list, allowed values for categorical fields, and conditional attributes
   ("if `category == "dress"`, also add `dress_type` …"). No domain specifics
   live in the engine.
2. **Text inference (torch + transformers).** The prompt is run through the
   model's chat template and a plain `AutoModelForCausalLM`. Decoding is greedy
   (`do_sample=False`) for reproducibility; prompt tokens are sliced off before
   decoding.
3. **Parse and validate.** Models are chatty, so `parse_json` extracts the first
   JSON object. `ExtractionSchema.validate` coerces each field: categoricals are
   lower/snake-cased and matched to the allowed set (off-list values kept,
   normalised); numbers/booleans coerced; empties become `null`.
4. **Emit (polars).** Validated dicts are concatenated onto the input frame.

The schema is the single source of truth for both the prompt and the validation,
which is what keeps the engine domain-agnostic: a new domain is a new schema, not
new engine code.

## How the modules compose

Each concept lives in its own module with co-located tests (Rust-style). The
Section-A modules share a convention that makes them **composable**: a
frame-level function (`*_frame`) takes a polars frame plus injected tools and
returns the same frame with new columns — later patterns (e.g. a routing
cascade) chain these functions without touching the modules. Every `*_frame`
also does its own Step-0 dedup: distinct descriptions are processed once and the
result broadcast to duplicate rows.

| Module          | What it does                                                                                                                                                                                                                                                                                      |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `schema/`       | The core abstraction. A declarative `ExtractionSchema` renders the prompt contract (`prompt_block`), the constrained-decoding contract (`json_schema`), and validates/coerces model output (`validate`: categorical snapping, number parsing, conditional sub-fields).                            |
| `domains/`      | The registered schemas: `generic` (one `product_type`), `clothing`, `electronics`. A schema is data — add one in python or load from JSON/YAML (`load_schema`).                                                                                                                                   |
| `extractor/`    | The LLM engine. `TextExtractor` runs the model through `transformers` (runs anywhere torch does). Also: the Qwen2.5 model registry, constrained decoding, grouped prompts, consistency voting.                                                                                                    |
| `pipeline/`     | Orchestration: `extract_features` — polars frame in, enriched frame out. Dispatches per-row / engine-batched / prompt-grouped, and wires Step-0 dedup, near-dedup and the result caches around any extractor.                                                                                     |
| `dedup/`        | Step 0. Normalised description keys (exact duplicates share one), near-duplicate merging by embedding similarity (greedy leader clustering), and a JSONL result cache. Scoped by (model, schema); failures are never cached.                                                                      |
| `embeddings/`   | Embeddings as features. `Encoder` (Qwen3-Embedding-0.6B by default, pooling resolved per checkpoint family) plus a persisted PCA projection (`fit_projection` / `apply_projection`) that compresses to a small fixed width. Surface: `embed_frame`.                                               |
| `cluster/`      | Embed → cluster → categorical. Greedy running-centroid clustering of raw-description embeddings; each cluster carries a medoid exemplar and distinctive TF-IDF terms so it can be named ONCE (`name_clusters`). `assign_to_centroids` is the incremental path. Surface: `cluster_frame`.          |
| `retrieval/`    | Zero-shot classification by retrieval. Embed every label once (gloss-expanded), assign descriptions by argmax cosine. Returns `similarity` and `margin` (best − runner-up) — a confidence signal a cascade can route on. Surface: `classify_frame`.                                               |
| `spans/`        | Span extraction: locate, don't generate. GLiNER tags the schema's fields as zero-shot entity types (optional `gliner` package, lazy import); the best span per field flows through `schema.validate`. Output columns match the LLM pipeline's. Surface: `locate_frame`.                           |
| `evaluation/`   | Scores predictions against a hand-labelled gold set: per-field support/accuracy, micro/macro. Pure — no model in the loop, so every path competes on the same bar.                                                                                                                               |
| `output/`       | The warehouse tables: L1/L2/L3, the extraction cache (`WarehouseExtractionCache` — same `get`/`put_many` interface as the JSONL cache), and the embedding + PCA-projection tables. Idempotent DDL; `init_all --recreate` rebuilds after a schema change.                                          |
| `deploy/`       | A Nimbus workflow spec builder (pure, tested) plus the vendored deploy client: builds the workflow image and registers the two-step hierarchy → condense workflow.                                                                                                                                  |
| `condense.py`   | Stage 2: incremental concept taxonomy over generated values (centroids, support gate, reconcile pass).                                                                                                                                                                                            |
| `discover.py`   | Taxonomy discovery: cluster generated values, report cardinality/entropy, emit a taxonomy file (which `extract classify --labels` accepts directly).                                                                                                                                              |
| `datasource.py` | Warehouse → `(id, description)` frame, with in-query anti-join and division/department filters.                                                                                                                                                                                                   |
| `database.py`   | Snowflake access: connector (OAuth / password), query rendering, batched writes.                                                                                                                                                                                                                  |

The tests need **no model or warehouse** — schemas are pure, the pipeline takes
its extractor by injection, encoders/locators/DBs are faked.

## The two-stage pipeline, and why it is decoupled

Generation is expensive; cleaning is cheap. The two stages are decoupled by a
buffer table so each is **independently runnable and incremental — never a full
refresh**.

```
  warehouse ──► extract hierarchy ──► L1 raw      (id, product_type free-text)
   (text)        Stage 1               │  incremental: anti-join on id
                                       ▼
                extract condense  ──► L2 concepts  (canonical taxonomy + centroids)
                 Stage 2          └─► L3 final     (id, canonical product_type)  ← ML-ready
```

- **L1 raw** — `extract hierarchy` samples products, generates `product_type`,
  and writes new rows. The anti-join on `id` happens **in the warehouse query**,
  so a sample of N yields N genuinely new products.
- **L2 concepts** — `extract condense` clusters the free-text values into
  canonical concepts, storing each concept's embedding **centroid** so a run
  only embeds the _new_ batch (O(new), not O(history)). A value that fits no
  existing concept seeds a `pending` one, promoted to `active` only after
  `--min-support` occurrences — one weird prediction can't spawn a concept.
- **L3 final** — per-product canonical `product_type` (`unallocated` until its
  concept is promoted; a reconcile pass re-places those each run).

`condense` keeps a single level (`product_type`) by default, but the machinery is
generalised over `condense.LEVELS`, so a nested taxonomy (e.g.
`segment → family → subtype`) is a one-line change.

## Why several paths

The job is to turn long free-text descriptions into something that **better
trains downstream ML models**. The original pipeline did this with a per-product
LLM generation step (Stage 1), which is too slow to scale — and not slow in a
way we can easily fix, because autoregressive decoding is inherently sequential
(one token at a time, per product). Two facts about the problem shape every path
the module offers.

**1. The cost is decoding, not encoding.** Stage 1 _generates_ text — the model
emits tokens one at a time, each a full forward pass. Embedding _encodes_ — a
single forward pass produces the whole vector, no decoding loop. That is why
`condense` is cheap and `hierarchy` is not. Any path that replaces generation
with encoding (or with a cheap classifier head) wins by a large constant.

**2. "Useful information" splits into two outputs that want different tools.**

- **(a) A categorical label** — the canonical `product_type`. Interpretable, a
  clean training _target_, joins to other tables. This is what the two-stage
  pipeline is built around.
- **(b) Features** — a dense representation of the description to use as model
  _inputs_. These need not be human-readable.

Embeddings answer (b); producing (a) cheaply is a different — and arguably more
valuable — problem. One wrinkle the split hides: the shipped schema is
**multi-field**, not a single label. It carries the category plus colour,
material, numeric fields, and conditional sub-fields. Single-label paths
(retrieval, the naming half of clustering) replace a _slice_ of Stage 1; span
extraction covers the remaining fields without generation.

The central lever, reframed: **the LLM does not have to run per product.** It can
run per _cluster_ (label a few thousand groups once), per _sample_ (teach a cheap
student), or not at all (retrieval / rules). Most of the design space is an
instance of moving or removing that per-product call.

## The paths the module provides

**Step 0 — deduplicate before anything (`dedup`).** Retail catalogues are full
of exact and near-duplicate descriptions (size/colour variants, the same product
from multiple sources). The module hashes exact duplicates (normalised
description keys; each distinct key processed once and broadcast — on by default),
optionally merges near-duplicates above a cosine threshold (greedy leader
clustering; a member inherits its leader's features), and caches results keyed on
(model, schema). The cache has two interchangeable backends behind one
`get`/`put_many` interface: an append-only JSONL for local runs and a warehouse
table for the pipeline, since workflow steps are ephemeral containers and the
warehouse is what persists between scheduled runs. Neither caches failures, so
failures stay retryable. Dedup is orthogonal to every path below — a free
multiplier on whatever else runs.

**Embeddings as direct features (`extract embed`).** Encode each description into
a vector and feed it straight to the downstream model — one forward pass per
product, no decoding, fully batchable on GPU. Compression is **PCA, not UMAP**,
deliberately: the projection must outlive the run, so tomorrow's products land in
the same space as today's stored vectors rather than silently shifting the
feature under the model. A PCA is a mean + a matrix — JSON-persisted (keyed by
encoder + dim), fitted once on the first run's sample, exact and deterministic on
out-of-sample rows, no new dependency (torch SVD). Consequence of the persisted
fit: make the first `extract embed` run a large, representative sample; `extract
init --recreate` drops embeddings and projection together.

**Embed → cluster → categorical (`extract cluster`).** Cluster the description
embeddings and use the cluster id as a categorical feature. This is almost
exactly what `condense` does — except `condense` clusters the **LLM's output
text**, having paid the generation cost first; clustering the **raw
descriptions** directly makes the expensive step vanish. The catch is naming: a
cluster id is not `"frying pan"`, but you only name **once per cluster** — a
medoid exemplar, distinctive TF-IDF terms, and/or one LLM call per cluster
(thousands of calls, not millions). The hidden bill is operational: new products
need assigning (kNN to the nearest centroid), clusters go stale and need
re-clustering on some cadence, and cluster names must stay stable across
re-clusterings or the feature silently changes meaning.

**Zero-shot classification by retrieval (`extract classify`).** Given a candidate
taxonomy (even the existing concepts), embed every **label name** once, embed
each description once, and assign by max cosine similarity — argmax over label
embeddings, no generation. It produces the categorical label directly and
cheaply, and extends to new categories with zero retraining. Accuracy depends on
label names being embedding-friendly, so each label can be expanded into a short
gloss ("pump — a slip-on court shoe") before embedding. The output carries
`similarity` and `margin` (best minus runner-up) per row — a ready-made
confidence signal for routing.

**Span extraction — locate, don't generate (`extract locate`).** Many attribute
values appear _verbatim_ in the description: colour, material, brand, weights and
dimensions. There is nothing to generate, only to locate. GLiNER (zero-shot NER)
tags the schema's field names as entity types in one forward pass per product;
the best span per field flows through `schema.validate`, so numbers parse out of
"2.5 kg" and categoricals snap to the choice list. This is the only
non-generative path that covers the schema's _non-category_ fields, so it
composes with clustering / retrieval to replace all of Stage 1 rather than a
slice of it.

## How to compare paths

The `evaluation` module scores predictions against a gold set with no model in
the loop, and every path emits the same column shapes — so comparisons are
apples-to-apples. Two rules, since the question is cost _vs_ quality: log cost
(throughput, $/1000 products) alongside accuracy, and report per field and per
class — a single global accuracy hides collapse on the tail classes, which is
exactly the slice a routing cascade would need to catch.
