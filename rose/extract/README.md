# extract

Schema-driven product feature extraction from **text + image** using a small
(4–8 GB) open-weight vision-language model (VLM) from Hugging Face, `torch` for
inference and `polars` for data movement.

Given a product description and a product photo, the pipeline returns a
structured set of attributes. Two domains ship out of the box and demonstrate
that the work is _abstract_ — the engine itself knows nothing about clothes or
electronics:

- **Clothing** — `category` (dress, top, …) → conditional `dress_type`
  (maxi / midi / wrap / …), plus colour, material, pattern, sleeve length, brand.
- **Electronics** — `category` (motherboard, gpu, cpu, …) → conditional
  `chipset` / `form_factor` / `socket` for a motherboard, `gpu_chip` / `vram_gb`
  for a GPU, and so on.

Adding a third domain (furniture, cosmetics, food, …) means writing a new
`ExtractionSchema` in `extract/domains.py`. No pipeline code changes.

---

## Architecture

```
                ┌──────────────────────────────────────────────┐
 products.csv → │  polars DataFrame  (id, description, image)   │
                └───────────────────────┬──────────────────────┘
                                        │  one row at a time
                       ┌────────────────▼─────────────────┐
   ExtractionSchema ──▶│  prompt synthesis (JSON contract) │
   (domains.py)        └────────────────┬─────────────────┘
                                        │  prompt + PIL image
                            ┌───────────▼───────────┐
                            │  VLM  (torch / HF)     │   image-text-to-text
                            └───────────┬───────────┘
                                        │  free text
                       ┌────────────────▼─────────────────┐
   ExtractionSchema ──▶│  JSON parse → validate / coerce   │
                       └────────────────┬─────────────────┘
                                        │  one dict per row
                ┌───────────────────────▼──────────────────────┐
   out.parquet ←│  polars DataFrame  (inputs + feature columns) │
                └───────────────────────────────────────────────┘
```

The package lives under `src/`, and tests are **co-located** Rust-style: each
module is a folder holding the code next to its test.

```
src/extract/
  schema/      schema.py      test_schema.py     # the abstraction (prompt out, validation in)
  domains/     domains.py     test_domains.py    # concrete schemas (clothing, electronics) — pure data
  extractor/   extractor.py   test_extractor.py  # VisionLanguageExtractor: VLM load, chat message, parse JSON
  pipeline/    pipeline.py    test_pipeline.py   # extract_features: polars frame in, enriched frame out
  cli/         cli.py         test_cli.py        # typer CLI: domains, describe, one, run
  scrape/      scrape.py      test_scrape.py     # extract-scrape: build a dataset from product pages (distinct from the engine)
  evaluation/  evaluation.py  test_evaluation.py # score predictions vs a gold set (per-field accuracy); pure scoring, no model
```

Each module folder's `__init__.py` re-exports its public API, so external
imports stay flat: `from extract.schema import ExtractionSchema`. The tests need
**no model** — the schema is pure, and the pipeline is driven through a _fake_
extractor (the pipeline takes its extractor by injection), so the suite runs in
seconds. The end-to-end demo lives outside the package at `examples/workflow.py`
(dataset → extraction → printed results).

---

## Methodology

The core idea is to turn a **generative** model into a **structured extractor**
by constraining both ends: a precise prompt going in, strict validation coming
out. The schema is the single source of truth for both.

1. **Ingest (polars).** Products arrive as a `DataFrame` with at least a
   `description` and an `image_path` column. polars is the data-movement layer
   end to end — it reads CSV/Parquet, iterates rows, and produces the enriched
   output frame. Conditional features (a dress has `dress_type`, a motherboard
   has `chipset`) naturally produce a _ragged_ result set; we materialise the
   output column-by-column against `schema.column_names()` so every row aligns
   and missing features become `null`.

2. **Prompt synthesis.** `ExtractionSchema.prompt_block()` renders the field
   list, the allowed values for categorical fields, and the _conditional_
   attributes ("if `category == "dress"`, also add `dress_type` …"). This is
   how the two-level hierarchy is expressed to the model without hard-coding
   anything domain-specific in the engine.

3. **Multimodal inference (torch + transformers).** The description and the
   image are packed into a chat message and run through the model's
   `apply_chat_template`. We load the checkpoint with
   `AutoModelForImageTextToText` — the generic class for this task family — so
   any compatible image-text-to-text checkpoint works behind the same code.
   Decoding is greedy (`do_sample=False`) for reproducibility, and we slice off
   the prompt tokens before decoding so only the generated answer is kept.

4. **Parse and validate.** Models are chatty, so `_parse_json` extracts the
   first JSON object from the output. `ExtractionSchema.validate` then coerces
   each field: categorical values are lower-cased / snake-cased and matched
   against the allowed set (off-list values are kept, normalised, so we can
   surface them downstream rather than silently dropping a more specific
   answer); booleans and numbers are coerced; empties become `null`. Crucially,
   a conditional child column is only populated when its parent value fired.

5. **Emit (polars).** The validated dicts are concatenated horizontally onto the
   input frame and written back as CSV, Parquet, or
   [Vortex](https://docs.vortex.dev/user-guide/polars). Vortex is read lazily
   (`vx.open(path).to_polars()` returns a `LazyFrame` with column pruning and
   predicate pushdown) and written from the Arrow view of the frame
   (`vx.io.write(df.to_arrow(), path)`). It is only imported when a `.vortex`
   path is actually used.

### Model selection

A registry of vetted open-weight VLMs ships in `extract.extractor.MODELS`,
spanning two families and a range of sizes so a job can target anything from a
laptop to a multi-GPU box:

- **Qwen2.5-VL** — `qwen2.5-vl-3b` (~7 GB at bf16; the default) through `-72b`.
- **Gemma 4** (gated on Hugging Face — accept the licence + `huggingface-cli
  login`) — the small on-device `gemma-4-e2b` / `gemma-4-e4b`, the
  Mixture-of-Experts `gemma-4-26b` (4B active), and the dense `gemma-4-31b`.

Every entry is multimodal and accepts image content in its chat template. They
load through one of two transformers auto-classes, recorded per entry as
`ModelChoice.loader`: `AutoModelForImageTextToText` (Qwen2.5-VL, larger Gemma 4)
or `AutoModelForMultimodalLM` (the small Gemma 4 E-series).

Pick a model by registry key, pass any raw Hugging Face id, or use `"auto"` —
`recommend_model()` selects the largest checkpoint that fits the detected device
memory (`available_memory_gb()`). `resolve(choice)` returns the full
`ModelChoice` (id + loader) to hand to `VisionLanguageExtractor`.

Device and precision are picked automatically: CUDA → `bfloat16`, Apple MPS →
`float16`, CPU → `float32`.

---

## Install & run

The project targets Python ≥ 3.11. With [uv](https://github.com/astral-sh/uv):

```bash
uv sync                       # install torch, transformers, polars, …
```

```bash
# Inspect a domain's extraction contract (no model needed)
uv run extract describe clothing

# List available domains
uv run extract domains

# List available models, flagging which fit this machine (--model defaults to 'auto')
uv run extract models

# Single product → JSON (downloads the model on first run)
uv run extract one electronics ./examples/images/motherboard.jpg \
    "ASUS ROG STRIX B650-E gaming motherboard, ATX, AM5"

# Batch a file of products → Parquet (or .csv / .vortex)
uv run extract run clothing products.csv out.parquet
uv run extract run clothing products.vortex out.vortex

# Or run the narrated end-to-end example workflow (defaults to products.csv)
uv run python examples/workflow.py --domain electronics

# Score extraction quality against a hand-labelled gold set (per-field accuracy)
uv run extract eval clothing examples/gold/clothing.csv
```

Run the test suite (no model needed — the pipeline is exercised through a fake
extractor):

```bash
uv run --group dev pytest
```

As a library:

```python
import polars as pl
from extract import VisionLanguageExtractor, extract_features, get_schema

df = pl.read_csv("examples/products.csv")
extractor = VisionLanguageExtractor()             # or VisionLanguageExtractor("Qwen/Qwen2-VL-2B-Instruct")
out = extract_features(df, get_schema("clothing"), extractor)
out.write_parquet("out.parquet")
```

### Scraping product pages

`extract/scrape.py` (the `extract-scrape` command) builds the input file from
live product pages, so you can go URL → features without hand-writing a CSV. It
is kept deliberately separate from
the extraction engine — its only job is to manufacture test datasets. It is
site-agnostic: it reads structured metadata in order of reliability — JSON-LD
(`@type: Product`), then Open Graph tags, then plain meta tags. As a last
resort it picks the `<img>` whose `alt` text best overlaps the product title
(falling back to the largest one) and resolves its highest-resolution URL from
`data-old-hires` / `srcset` — this reliably selects the product photo over
borders, logos and related-item thumbnails. Output columns match the pipeline
(`id`, `description`, `image_path`).

```bash
uv run extract-scrape \
    "https://books.toscrape.com/catalogue/a-light-in-the-attic_1000/index.html" \
    --out products.csv --image-dir images
uv run extract run clothing products.csv out.parquet
```

`products.csv` feeds the pipeline directly: each row uses its local
`image_path` when present, and otherwise falls back to the scraped remote
`image_url` (the model loads it over HTTP). So the scraped file runs as-is even
if you skip the image download.

It honours `robots.txt` by default (`--ignore-robots` to override) and waits
`--delay` seconds between requests. Keep volumes low. Large retailers such as
Amazon often block non-browser traffic or serve a CAPTCHA; permissive sandboxes
like [books.toscrape.com](https://books.toscrape.com) are the reliable choice
for a demo.

### Adding a domain

```python
from extract.schema import ExtractionField as F, ExtractionSchema, FieldKind

FURNITURE = ExtractionSchema(
    domain="furniture",
    description="home furniture",
    fields=(
        F("category", "type of furniture", FieldKind.CATEGORICAL,
          ("chair", "table", "sofa"),
          children={"chair": (F("chair_style", "style", FieldKind.CATEGORICAL,
                                ("dining", "office", "lounge")),)}),
        F("primary_material", "main material"),
    ),
)
```

Register it in `SCHEMAS` and it is immediately available to the CLI and pipeline.
Or skip Python entirely and load a declarative schema from a file — no engine
changes needed:

```python
from extract.schema import load_schema

furniture = load_schema("furniture.yaml")  # or .json
```

---

## Capabilities

- **Optional image.** The description is required; an image is used when present
  (`image_path`/`image_url`), and rows extract from text alone otherwise.
- **Batched inference.** `--batch-size N` (or `extract_features(..., batch_size=N)`)
  runs the model on left-padded batches to amortise per-call overhead.
- **Constrained decoding (optional).** `VisionLanguageExtractor(..., constrained=True)`
  enforces the schema's `json_schema()` via `outlines` (install separately) so
  the model cannot emit invalid JSON or off-taxonomy categoricals.
- **Confidence / abstention.** `extract_consistent(...)` samples N times and
  majority-votes each field, returning a per-field agreement score.
- **Quantisation.** `VisionLanguageExtractor(..., quantization="4bit")` loads
  bitsandbytes-quantised weights (needs a CUDA GPU + `bitsandbytes`).
- **Reproducibility & provenance.** Pin a model commit with `--revision`; `run`
  stamps `_model` / `_schema` / `_extracted_at` columns (disable with
  `--no-provenance`).
- **Evaluation.** `extract eval <domain> <gold.csv>` scores per-field accuracy
  against a labelled gold set.

## Limitations

- **Off-list categorical values are kept, not rejected** (unless `constrained=True`).
  Good for recall, but the column can hold values outside the declared taxonomy.
- **Batched mode trades per-row error isolation for speed** — a failed batch
  nulls its rows (logged) rather than isolating the single bad row.
- **The `outlines` and `bitsandbytes` paths are optional and hardware-dependent**
  — wired but validated on GPU, not in the test suite.

---

## Future enhancements

- **Text-only fast path.** Every model in the registry is vision-language and
  carries a vision tower even on text-only rows. For datasets (or rows) with no
  image, a dedicated **text-to-text** LLM would likely be faster and lighter for
  the same quality — worth a `loader="text"` registry entry and routing
  image-less rows to it. The schema/prompt/validation layers are already
  modality-agnostic, so only the model class and per-row routing change.
- **Text/image agreement.** Extract independently from the description and from
  the image, then reconcile — disagreement is a strong signal of a mislabelled
  listing or a stock-photo mismatch.
- **Deeper / external taxonomies.** The schema currently expresses two levels;
  generalise `children` to arbitrary depth and load allowed values from an
  external product taxonomy (e.g. Google Product Taxonomy, GS1) rather than
  hard-coding them.
- **Spec-label OCR for electronics.** Components often carry the exact part
  number on a printed label; a dedicated OCR pass can feed verified strings into
  the prompt and sharpen fields like `chipset` and `model`.
- **Async / service mode.** Wrap the extractor behind an async queue or a small
  service so it can be driven from an ingestion pipeline.
- **Result caching.** Hash (image + description + schema version) to skip
  re-extracting unchanged products on re-runs.
