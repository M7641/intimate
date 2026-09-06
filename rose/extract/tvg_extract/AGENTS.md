# AGENTS.md — authoring `extract` workflows

A recipe sheet for LLMs/agents writing extraction jobs against this package.
The engine turns a **generative text LLM** into a **structured extractor** (a
schema drives the prompt and validates the output), and ships a family of
**cheaper non-generative paths** (embed / cluster / classify / locate) behind
one composable convention. Everything is `polars` in → enriched `polars` out.
The warehouse pipeline is **incremental — never a full refresh**. Text only —
there is no image path. Usage: `README.md`; design rationale: `docs/design.md`;
directions not yet built: `docs/directions.md`.

## CLI (the fastest path)

```bash
# Stage 1 — generate a free-text product_type from the warehouse (new ids only)
uv run extract init
uv run extract hierarchy --sample 1000 --division Womenswear --cache

# Stage 2 — condense the free text into a canonical concept taxonomy + final table
uv run extract condense --min-support 3

# Section A — non-generative paths (no decoding loop anywhere)
uv run extract embed    --sample 2000 --dim 16          # A1: vectors → warehouse
uv run extract cluster  in.csv out.csv --clusters c.json --name-model qwen2.5-0.5b  # A2
uv run extract classify in.csv out.csv --labels taxonomy.json                       # A3
uv run extract locate   clothing in.csv out.csv         # A4 (needs `uv pip install gliner`)

# Analysis / one-offs
uv run extract discover --threshold 0.6 --output taxonomy.json  # cluster + entropy
uv run extract run generic in.csv out.parquet                   # enrich a file
uv run extract one generic "Le Creuset cast iron frying pan, 28cm"  # single product
uv run extract eval clothing gold.csv  # ·  extract domains  ·  extract models
```

The CLI is one module per pattern under `cli/` (`warehouse.py`, `embed.py`,
`cluster.py`, `classify.py`, `locate.py`, `llm.py`, `deploy.py`, `info.py`);
`cli/cli.py` only assembles them onto the typer app, and `cli/common.py` holds
the shared console/IO/help-string plumbing.

## The two-stage pipeline (three layers)

The LLM generates *free text* — noisy for ML (synonyms, spellings). So generation
and a cheap, incremental condensing step are separated by a buffer table:

- **L1 raw** — `datasource.load_products(sample_size=N, exclude_schema, exclude_table, division, department)`
  joins the staging tables at **product grain** and returns `id`, `description`.
  The already-stored anti-join and the filters run **in the warehouse query**,
  so `--sample N` is N genuinely-new products. `extract hierarchy` writes the
  free-text `product_type` to `llm_product_hierarchy_proto`.
  See `input_data_layer.dbml`.
- **L2 concepts** — `extract condense` clusters L1 text into a canonical taxonomy
  (`llm_hierarchy_concept_proto`): each concept stores a `centroid`, so a run
  only embeds the *new* batch and assigns it against stored vectors. A
  non-matching value seeds a `pending` concept, **promoted** to `active` only
  after `min_support` occurrences (the gate) — one-off noise never spawns a
  concept.
- **L3 final** — `extract condense` writes per-product **canonical** labels to
  `llm_product_hierarchy_final_proto` (the ML table), `unallocated` where a
  concept isn't promoted yet. A **reconcile** pass each run re-places parked rows
  against newly-promoted concepts (read-only on the tree).

`output/` functions: `init_all` (every table; `--recreate` after a schema
change), `init_table` / `init_concept_table` / `init_final_table` /
`init_cache_table` / `init_embedding_table` / `init_projection_table` (DDL),
`load_unmapped` (new batch), `load_concepts` / `save_concepts`,
`write_hierarchy` (L1), `write_final` / `load_unallocated` / `update_final`
(L3), `WarehouseExtractionCache` (Step-0 cache), `load_projection` /
`save_projection` / `write_embeddings` / `stored_embedding_models` (A1).
The pure clustering logic lives in `condense.py` (`condense_batch`,
`reconcile_batch`) — torch-free and unit-tested; embedding is isolated in
`embed_values`. It is generalised over `condense.LEVELS` (a single `product_type`
today; a nested tree is a one-line change). See `output_data_layer.dbml`.

## The composable Section-A paths

Each non-generative path is its own module exposing a **frame-level surface**:
a polars frame plus injected tools in, the same frame with new columns out.
Every surface does Step-0 dedup internally (distinct descriptions processed
once, result broadcast to duplicate rows). Compose them by chaining calls —
e.g. a D1 cascade = `classify_frame`, filter on `margin`, send the residual
through `extract_features`.

```python
from extract.embeddings import Encoder, embed_frame
from extract.cluster import cluster_frame, name_clusters
from extract.retrieval import classify_frame, load_labels
from extract.spans import GLiNERLocator, locate_frame

enc = Encoder()                      # Qwen3-Embedding-0.6B; pooling auto-resolved

out, projection = embed_frame(df, enc, dim=16)             # A1 → `embedding`
out, clusters = cluster_frame(df, enc, threshold=0.65)     # A2 → `cluster_id` (+ exemplars/terms)
out = classify_frame(df, enc, load_labels("taxonomy.json"))  # A3 → label + `similarity` + `margin`
out = locate_frame(df, GLiNERLocator(), schema)            # A4 → one column per schema field
```

Per-path notes:

- **A1 `embeddings/`** — the PCA projection is DATA: fitted once on the first
  run, persisted in the warehouse (`load_projection`/`save_projection`), and
  reused so every run lands in the same space. Make the first run a large,
  representative sample (≥ `dim` distinct descriptions; thousands better).
- **A2 `cluster/`** — clusters carry a medoid `exemplar` and distinctive
  `terms`; name each cluster ONCE via `name_clusters(clusters, label_fn)` —
  one LLM call per cluster, not per product. `assign_to_centroids` routes new
  products incrementally.
- **A3 `retrieval/`** — labels embed best with a gloss (`{"name": "pump",
  "gloss": "a slip-on court shoe"}`); `load_labels` also accepts a plain name
  list or an `extract discover` taxonomy file. Keep the `margin` column — it
  is the D1 routing signal.
- **A4 `spans/`** — GLiNER is optional (`uv pip install gliner`), lazy-imported;
  tests inject `GLiNERLocator(model=fake)`. Spans flow through
  `schema.validate`, so output columns match the LLM pipeline's exactly.
- **Encoder** — `Encoder(model_id, pooling=None)`: pooling (`last`/`cls`/`mean`)
  resolves from the checkpoint family; returns float32 unit rows. Vectors from
  different encoders are NOT comparable — `extract embed` refuses to mix them.
  (`discover`/`condense` keep their own MiniLM: their stored centroids live in
  its space.)

## Step-0 dedup & caches (on every extraction)

`extract_features` dedups by default: rows are keyed on the normalised
description, each distinct key is extracted once, results broadcast. Knobs
(library kwargs = CLI flags):

- `dedup=True` — exact dedup; identical output, less compute (`--no-dedup`).
- `near_threshold=0.95` — also merge near-duplicates by embedding cosine;
  members inherit their leader's features (an approximation — opt-in).
- `cache=` — an object with `get`/`put_many`: `dedup.ExtractionCache` (local
  JSONL, `extract run --cache results.jsonl`) or
  `output.WarehouseExtractionCache` (`extract hierarchy --cache` — survives
  ephemeral workflow containers). Scoped by (model, schema); failures are
  never cached, so they stay retryable.

## Throughput levers (the LLM path)

- `--batch-size N` — engine batching; `0` = the whole input in ONE call. A
  failed batch nulls its rows.
- `--group-size N` — prompt batching (C3): N descriptions in one prompt, a
  JSON array of exactly N objects back, per-row fallback on misalignment.
  Mutually exclusive with `--batch-size != 1` (different layers: prompt vs
  engine).
- `--constrained` — JSON-schema guided decoding via the optional `outlines`
  package; in grouped mode the contract is an array schema pinned to the group
  size.
- `--quantization 4bit` — bitsandbytes (CUDA).

## The workflow, end to end (library)

```python
from extract import TextExtractor, extract_features, get_schema
from extract.database import DBActions
from extract.datasource import load_products
from extract.output import WarehouseExtractionCache, init_table, write_hierarchy

db = DBActions()
init_table(db)                                  # idempotent DDL

# 1. Load NEW products (warehouse anti-join + optional filters).
df = load_products(
    sample_size=50,
    exclude_schema="sandpit",
    exclude_table="llm_product_hierarchy_proto",
    division="Womenswear",                      # or None
)                                               # → id, description

# 2. Build the schema, load a model ("auto" = largest that fits this machine).
schema = get_schema("generic")                  # generates `product_type`
extractor = TextExtractor("qwen2.5-1.5b")       # any registry key or HF text-LLM id

# 3. Extract: one feature column per schema field, appended to the frame.
#    Dedup is on by default; the cache makes repeat runs skip known text.
cache = WarehouseExtractionCache(db, model_id=extractor.model_id, schema_domain=schema.domain)
out = extract_features(df, schema, extractor, batch_size=8, cache=cache)

# 4. Store the new rows (records model, schema_domain, created_at, updated_at).
write_hierarchy(db, out, model_id=extractor.model_id, schema_domain=schema.domain)
db.close()
```

## Generating metadata (provenance)

`stamp_provenance(df, *, model_id, schema_domain, at)` appends three columns so
a result frame records **what** produced it and **when**:

| column          | value                                          |
| --------------- | ---------------------------------------------- |
| `_model`        | the Hugging Face id (pin a revision: `id@rev`) |
| `_schema`       | `schema.domain`                                |
| `_extracted_at` | an ISO-8601 timestamp **you pass in**          |

It is intentionally **pure**: pass the timestamp (and a pinned `model_id`) from
the caller so runs stay deterministic and reproducible. The CLI `run` command
does this automatically unless `--no-provenance`. The `hierarchy` command does
**not** use this — `write_hierarchy` records the equivalent metadata as plain
table columns (`model` / `schema_domain` / `created_at` / `updated_at`).

## Model registry & selection

`extract.extractor.MODELS` maps a short key → `ModelChoice(model_id,
params, min_memory_gb, note)`. A clean text-to-text ladder (Qwen2.5 Instruct):
`qwen2.5-0.5b` / `-1.5b` / `-3b` / `-7b` / `-14b` / `-32b` / `-72b`. `extract
hierarchy` defaults to `qwen2.5-1.5b`.

- `resolve(choice)` → a `ModelChoice`. `choice` is `"auto"`, a registry key, or a
  raw HF id. Hand `choice.model_id` to `TextExtractor`.
- `recommend_model(memory_gb=None)` → the registry **key** of the largest model
  that fits; auto-detects memory when omitted.
- `available_memory_gb()` → device memory ceiling (CUDA VRAM, else system RAM).
- `min_memory_gb` is a guide for selection, not a hard gate — a model can still
  run below it via CPU offload.

The Section-A encoder is separate: `embeddings.DEFAULT_ENCODER`
(`Qwen/Qwen3-Embedding-0.6B`), overridable per command with `--encoder`.

## How it runs on a GPU

Device and precision are auto-selected by `select_device()` / `dtype_for()`:

| hardware      | device | dtype      |
| ------------- | ------ | ---------- |
| CUDA GPU      | `cuda` | `bfloat16` |
| Apple Silicon | `mps`  | `float16`  |
| CPU only      | `cpu`  | `float32`  |

On a CUDA box nothing changes in your code:

- **Auto-selection scales up.** `available_memory_gb()` returns the GPU's total
  VRAM, so `resolve("auto")` picks a larger model (e.g. a 24 GB GPU →
  `qwen2.5-7b`/`-14b`; an 80 GB A100 → `qwen2.5-32b`).
- **Weights land on the GPU** in `bfloat16` and inference runs there — typically
  one to two orders of magnitude faster than CPU.
- **Throughput:** raise `--batch-size`, which the GPU amortises well. Batches
  left-pad; a failed batch nulls its rows.
- **Bigger than VRAM?** Use `quantization="4bit"` (bitsandbytes, CUDA-only) to
  roughly quarter the memory, or rely on `device_map="auto"` offload (slower).

## Defining a new domain

Schemas are data, not code. Add an `ExtractionSchema` to
`src/extract/domains/domains.py` and register it in `SCHEMAS`, or
load a declarative file with `load_schema("furniture.yaml")`. Fields:
`FieldKind.{CATEGORICAL,TEXT,BOOLEAN,NUMBER,INTEGER}`; categorical fields need
`choices`; `children` unlock conditional sub-fields when a parent value fires.

## Gotchas

- The **description is required**; extraction is text-only.
- Decoding is **greedy** (`do_sample=False`) for reproducibility. Use
  `extractor.extract_consistent(...)` for a per-field agreement/confidence
  score; `classify_frame` returns a cosine `margin` per row.
- Off-list categorical values are **kept (normalised), not rejected** — unless
  `constrained=True` (`--constrained`; needs the optional `outlines` package).
- `batch_size` and `group_size` are **mutually exclusive** (`!= 1` together
  raises) — they batch at different layers.
- Output columns are ragged-safe: every `schema.column_names()` column exists;
  missing/failed values are `null`. A4's `locate_frame` abstains to `null`
  the same way.
- The A1 PCA projection and A2 cluster names are **stateful**: refitting or
  re-clustering silently changes feature meaning downstream. The projection
  persists in the warehouse; switching encoders requires
  `extract init --recreate`.
- Dedup is **on by default** in `extract_features` — when counting model
  calls in tests or budgets, count DISTINCT normalised descriptions, not rows.
- Tests need **no model, no warehouse** — extractors, encoders, locators and
  DBs are all taken by injection; pass fakes. See the co-located `test_*.py`
  next to each module.
