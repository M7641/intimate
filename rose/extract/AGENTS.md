# AGENTS.md — authoring `extract` workflows

A recipe sheet for LLMs/agents writing extraction jobs against this package.
Mirrors `examples/workflow.py`. The engine turns a **generative** VLM into a
**structured extractor**: a schema drives the prompt and validates the output.
Everything is `polars` in → enriched `polars` out.

## The workflow, end to end

```python
import datetime
import polars as pl
from extract import VisionLanguageExtractor, extract_features, get_schema
from extract.extractor import resolve          # model registry + auto-select
from extract.pipeline import stamp_provenance  # metadata columns

# 1. Load products (needs a `description`; `image_path`/`image_url` optional).
df = pl.read_csv("examples/products.csv")

# 2. Build the schema for a domain ("clothing", "electronics", or your own).
schema = get_schema("electronics")

# 3. Pick + load a model. "auto" = largest registry model that fits the machine.
choice = resolve("auto")                        # or resolve("gemma-4-e4b")
extractor = VisionLanguageExtractor(choice.model_id, loader=choice.loader)

# 4. Extract: one feature column per schema field, appended to the frame.
out = extract_features(df, schema, extractor)   # batch_size=N for throughput

# 5. Stamp provenance metadata so the output is self-describing (see below).
out = stamp_provenance(
    out,
    model_id=choice.model_id,
    schema_domain=schema.domain,
    at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
)

# 6. Persist (.csv / .parquet / .vortex by suffix).
out.write_parquet("out.parquet")
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
does this automatically unless `--no-provenance`.

## Model registry & selection

`extract.extractor.MODELS` maps a short key → `ModelChoice(model_id, params,
min_memory_gb, note, loader)`. Two families, all multimodal:

- **Qwen2.5-VL** — `qwen2.5-vl-3b` / `-7b` / `-32b` / `-72b`.
- **Gemma 4** (gated on HF — accept the licence + `huggingface-cli login`) —
  `gemma-4-e2b` / `gemma-4-e4b` (small on-device E-series) /
  `gemma-4-26b` (MoE, 4B active) / `gemma-4-31b` (dense).

Helpers:

- `resolve(choice)` → a full `ModelChoice`. `choice` is `"auto"`, a registry
  key, or a raw HF id (passed through with the default loader).
- `recommend_model(memory_gb=None)` → the registry **key** of the largest model
  that fits; auto-detects memory when `memory_gb` is omitted.
- `available_memory_gb()` → device memory ceiling (CUDA VRAM, else system RAM).

**Always pass `loader=choice.loader`** to `VisionLanguageExtractor`. The small
Gemma 4 E-series load via `AutoModelForMultimodalLM`; everything else via
`AutoModelForImageTextToText`. `loader` defaults to `"image-text-to-text"`, so
omitting it silently breaks the E-series. `min_memory_gb` is a guide for
selection, not a hard gate — a model can still run below it via CPU offload.

## How it runs on a GPU

Device and precision are auto-selected by `select_device()` / `_dtype_for()`:

| hardware      | device | dtype      |
| ------------- | ------ | ---------- |
| CUDA GPU      | `cuda` | `bfloat16` |
| Apple Silicon | `mps`  | `float16`  |
| CPU only      | `cpu`  | `float32`  |

On a CUDA box nothing changes in your code:

- **Auto-selection scales up.** `available_memory_gb()` returns the GPU's total
  VRAM, so `resolve("auto")` picks a larger model (e.g. a 24 GB GPU →
  `qwen2.5-vl-7b`; an 80 GB A100 → `gemma-4-31b` / `qwen2.5-vl-32b`).
- **Weights land on the GPU** in `bfloat16` and inference runs there — typically
  one to two orders of magnitude faster than CPU.
- **Throughput:** raise `batch_size` (`extract_features(..., batch_size=N)`),
  which the GPU amortises well. Batches left-pad; a failed batch nulls its rows.
- **Bigger than VRAM?** Use `quantization="4bit"` (bitsandbytes, CUDA-only) to
  roughly quarter the memory, or rely on `device_map="auto"` offload (slower).
- **Multi-GPU / very large models** (26B-A4B, 31B, 72B): the quantised path uses
  `device_map="auto"` (accelerate shards across visible GPUs); don't `.to()` it
  by hand — the wrapper already avoids that for the quantised branch.

## Defining a new domain

Schemas are data, not code. Add an `ExtractionSchema` to
`src/extract/domains/domains.py` and register it in `SCHEMAS`, or load a
declarative file with `load_schema("furniture.yaml")`. Fields:
`FieldKind.{CATEGORICAL,TEXT,BOOLEAN,NUMBER,INTEGER}`; categorical fields need
`choices`; `children` unlock conditional sub-fields when a parent value fires.

## Gotchas

- The **description is required**; the image is optional (used when
  `image_path`/`image_url` is present, else text-only).
- Decoding is **greedy** (`do_sample=False`) for reproducibility. Use
  `extractor.extract_consistent(...)` for a per-field agreement/confidence score.
- Off-list categorical values are **kept (normalised), not rejected** — unless
  `constrained=True` (needs the optional `outlines` package).
- Output columns are ragged-safe: every `schema.column_names()` column exists;
  missing/failed values are `null`.
- Tests need **no model** — the pipeline takes its extractor by injection, so
  pass a fake. See `src/extract/extractor/test_extractor.py`.

## Future: a text-only fast path

The registry today is all **vision-language** models, loaded for every row even
when a row has no image. When a dataset (or a given row) is **text-only**, a
dedicated **text-to-text** LLM would likely be faster and cheaper for the same
quality: no vision tower to load or run, smaller memory footprint, higher
throughput. Worth adding a text-only model option to the registry and routing
image-less rows to it — e.g. a `loader="text"` (`AutoModelForCausalLM`) entry,
or splitting the frame and running a text model on the image-less partition.
The schema/prompt/validation layers are already modality-agnostic, so only the
extractor's model class and the pipeline's per-row routing would change.
