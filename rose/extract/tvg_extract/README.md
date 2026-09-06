# extract

Schema-driven structured extraction from product **text**, using small
open-weight models from Hugging Face, `torch` for inference and `polars` for
data movement. It turns a product's free-text description into ML-ready features
— either by generating structured fields with an LLM, or via cheaper
non-generative paths (embeddings, clustering, retrieval, span extraction).

This README is the usage guide. For how it works and why it is built this way,
see [`docs/design.md`](docs/design.md); for directions not yet built, see
[`docs/directions.md`](docs/directions.md).

## Install

This is a **self-contained** project: everything lives under `extract/`, so you
`cd` into it and run [uv](https://github.com/astral-sh/uv) from there.

**1. Install uv** (once, if you don't have it):

```bash
# macOS / Linux
curl -LsSf https://astral.sh/uv/install.sh | sh

# or with Homebrew
brew install uv

# or with pipx
pipx install uv
```

See the [uv install docs](https://docs.astral.sh/uv/getting-started/installation/)
for Windows and other options.

**2. Sync the project.** uv reads `.python-version` (3.12) and `uv.lock`, creates
a virtualenv, and installs everything — no separate Python or venv step:

```bash
cd extract
uv sync                       # installs torch, transformers, polars, …
uv run extract --help         # confirm the CLI resolves
```

`uv run <cmd>` runs inside the project's environment; you never activate a venv
by hand. Optional extras, installed only when you use the path that needs them:

```bash
uv pip install gliner         # for `extract locate` (span extraction)
uv pip install outlines       # for `--constrained` (JSON-schema guided decoding)
```

## Quick start (no warehouse needed)

```bash
uv run extract domains                 # list domains
uv run extract describe generic        # the extraction contract (prompt) for a domain
uv run extract models                  # registry, flagging which fit this machine

# Single product description → JSON (downloads the model on first run)
uv run extract one generic "Le Creuset cast iron frying pan, 28cm"

# Enrich a file of products (csv/parquet) — each row from its `description`
uv run extract run generic products.csv out.parquet --batch-size 8

# Score any path's output against a hand-labelled gold set (per-field accuracy)
uv run extract eval clothing path/to/gold.csv
```

## The two-stage warehouse pipeline

Needs `SNOWFLAKE_*` / OAuth `API_KEY` in the environment. Each stage is
incremental (anti-join on `id`) — never a full refresh.

```bash
uv run extract init                          # once: create the output tables
uv run extract hierarchy --sample 1000 --division Womenswear   # Stage 1: generate product_type
uv run extract condense  --min-support 3     # Stage 2: → concept taxonomy + final table
uv run extract discover  --threshold 0.6     # inspect cardinality / entropy
```

- `--sample N` pulls N genuinely new products.
- `--division Womenswear` / `--department FURNITURE` restrict the run.
- `--cache` (on `hierarchy`) persists results in the warehouse so scheduled,
  ephemeral workflow runs reuse them.

## The non-generative paths

```bash
uv run extract embed    --sample 2000 --dim 16            # compressed embeddings + persisted PCA
uv run extract cluster  products.csv out.csv --clusters clusters.json --name-model qwen2.5-0.5b
uv run extract classify products.csv out.csv --labels taxonomy.json   # emits `similarity` + `margin`
uv run extract locate   clothing products.csv out.csv     # GLiNER spans; needs `uv pip install gliner`
```

`extract embed` takes `--sample N` or `--all` (embed every new product in the
chosen `--division`/`--department` in one sweep). It fits the PCA projection on
the **first** run and reuses it forever after, so later products land in the
same space — give the first run a large, representative sample. Switching
encoders means clearing the embedding table (`extract init --recreate`); vectors
from different encoders are not comparable.

## CLI reference

| Command                                   | What it does                                                      |
| ----------------------------------------- | ----------------------------------------------------------------- |
| `extract init`                            | Create (or `--recreate`) the warehouse tables                     |
| `extract hierarchy`                       | Stage 1: generate `product_type` from the warehouse (LLM)         |
| `extract condense`                        | Stage 2: condense raw values into the concept taxonomy            |
| `extract discover`                        | Inspect/emit a taxonomy from generated values                     |
| `extract embed`                           | Store compressed description embeddings (+ persisted PCA)         |
| `extract cluster`                         | Cluster raw descriptions; name clusters once (`--name-model`)     |
| `extract classify`                        | Zero-shot label by retrieval (`--labels`, emits `margin`)         |
| `extract locate`                          | GLiNER span extraction against a domain schema                    |
| `extract run`                             | Enrich a local csv/parquet with the LLM extractor                 |
| `extract one`                             | One description → JSON, for poking at a schema/model              |
| `extract eval`                            | Score any path's output against a gold set                        |
| `extract deploy`                          | Build + register the Nimbus workflow                              |
| `extract domains` / `models` / `describe` | Introspection (no model, no warehouse)                            |

Every command takes `--help`.

## Levers on the LLM path

These flags apply to `run`, `hierarchy` (and where noted, `one` / `eval`):

| Flag                  | Effect                                                                                       |
| --------------------- | ------------------------------------------------------------------------------------------- |
| `--model`             | A registry key (`qwen2.5-1.5b`), any HF text-LLM id, or `auto` (largest that fits memory).  |
| `--batch-size N`      | Run left-padded batches through one `generate()`. `0` = the whole input in one call.        |
| `--group-size N`      | Pack N descriptions into one prompt (array reply, per-row fallback). Excludes `--batch-size != 1`. |
| `--constrained`       | JSON-schema guided decoding via `outlines` — guarantees valid JSON, prunes the token space. |
| `--quantization 4bit` | bitsandbytes 4-bit weights (CUDA) — roughly quarters the memory.                             |
| `--no-dedup`          | Disable Step-0 dedup (distinct descriptions are otherwise extracted once and broadcast).    |
| `--near-threshold X`  | Also merge near-duplicate descriptions above cosine X (members inherit the leader's result).|
| `--cache`             | Persist results keyed by (model, schema) — JSONL locally, warehouse table for `hierarchy`.  |
| `--revision`          | Pin a model commit for reproducibility.                                                      |

Model device/precision is auto-selected: CUDA → `bfloat16`, Apple MPS →
`float16`, CPU → `float32`. `TextExtractor` runs anywhere torch does. See
`uv run extract models` for the registry and which checkpoints fit this machine.

## Defining a schema

A schema is data, not code. The `generic` schema is a single free-text field:

```python
GENERIC = ExtractionSchema(
    domain="product type",
    description="the specific product type for a retail product",
    fields=(F(name="product_type", description="the most precise noun for what the product IS"),),
)
```

`clothing` and `electronics` show the richer framework — `CATEGORICAL` fields
with `choices`, and `children` that unlock conditional sub-fields:

```python
from extract.schema import ExtractionField as F, ExtractionSchema, FieldKind

FURNITURE = ExtractionSchema(
    domain="furniture",
    description="home furniture",
    fields=(
        F("category", "type of furniture", FieldKind.CATEGORICAL, ("chair", "table", "sofa"),
          children={"chair": (F("chair_style", "style", FieldKind.CATEGORICAL,
                                ("dining", "office", "lounge")),)}),
        F("primary_material", "main material"),
    ),
)
```

Register it in `SCHEMAS` and it is available to the CLI and pipeline — or load a
declarative schema from a file with `load_schema("furniture.yaml")`.

## As a library

```python
from extract import TextExtractor, extract_features, get_schema
from extract.datasource import load_products

df = load_products(sample_size=50)        # warehouse → id, description
extractor = TextExtractor()               # default: qwen2.5-0.5b; or pass any HF text-LLM id
out = extract_features(df, get_schema("generic"), extractor)
out.write_parquet("out.parquet")
```

## Tests

No model or warehouse needed — schemas are pure, the pipeline takes its extractor
by injection, encoders/locators/DBs are faked. The suite runs in seconds:

```bash
uv run --group dev pytest
```
