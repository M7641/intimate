"""Shared CLI plumbing: console, progress, IO, model loading, help strings.

Every command module imports from here; nothing here imports a command
module, so the dependency between the CLI files only points one way.
"""

from __future__ import annotations

import logging
from pathlib import Path

import polars as pl
import typer
from rich.console import Console
from rich.progress import (
    BarColumn,
    MofNCompleteColumn,
    Progress,
    TextColumn,
    TimeRemainingColumn,
)

from extract.extractor import (
    MODELS,
    TextExtractor,
    available_memory_gb,
    resolve,
)

console = Console()

# --- shared --model option ---------------------------------------------------

MODEL_HELP = (
    "model to use: 'auto' (largest registry model that fits this machine), a "
    f"registry key ({', '.join(MODELS)}), or any Hugging Face text-LLM id. "
    "See `extract models`."
)

# --- Group-C throughput levers (see README) ----------------------------------

CONSTRAINED_HELP = (
    "JSON-schema guided decoding: guarantees valid JSON and prunes the token "
    "space (needs the optional `outlines` package)."
)
QUANTIZATION_HELP = (
    "weight quantization: '4bit' (transformers + CUDA only); blank = full precision."
)
BATCH_HELP = (
    "rows per batched generate() call — the throughput lever (try 8-16 on a GPU); "
    "0 = the whole input in ONE call."
)
GROUP_HELP = (
    "descriptions per prompt (C3 prompt batching): the schema contract is paid "
    "once per group, with per-row fallback on a misaligned reply. Mutually "
    "exclusive with --batch-size != 1."
)

# --- Step-0 dedup levers (see README) ----------------------------------------

DEDUP_HELP = (
    "extract each distinct description once and broadcast the result — same "
    "output, less compute (Step 0). On by default."
)
NEAR_HELP = (
    "also merge NEAR-duplicate descriptions at this embedding cosine "
    "similarity (e.g. 0.95); a member inherits its group leader's features — "
    "an approximation, so off by default (0 = off)."
)
CACHE_HELP = (
    "local JSONL file caching results per description across runs (keyed by "
    "model + schema); cached descriptions skip the model entirely."
)
WAREHOUSE_CACHE_HELP = (
    "cache results per description in the warehouse "
    "(sandpit.llm_extraction_cache_proto, keyed by model + schema) — workflow "
    "steps are ephemeral, so this is the cache that persists between scheduled "
    "runs; cached descriptions skip the model entirely."
)

# --- Section-A encoder options (embed/cluster/classify) ----------------------

DEFAULT_ENCODER_ID = "Qwen/Qwen3-Embedding-0.6B"
ENCODER_HELP = (
    "sentence-encoder checkpoint — any Hugging Face id; the default is the "
    "Qwen3 embedding model (1024-dim, 32k context, same family as the "
    "extractor's Qwen2.5)."
)
POOLING_HELP = (
    "pooling override: 'last', 'cls' or 'mean'; blank = resolved from the "
    "checkpoint name (last for qwen3-embedding, cls for "
    "bge/gte/arctic/modernbert, mean otherwise)"
)
ENCODER_BATCH_HELP = "texts per encoder forward pass"

# --- warehouse scoping (hierarchy/embed/deploy) -------------------------------

DIVISION_HELP = "restrict to one product_division (e.g. 'Womenswear')"
DEPARTMENT_HELP = "restrict to one product_department (e.g. 'FURNITURE')"


def configure_logging() -> None:
    """Keep the terminal short: our warnings only; hush HTTP / hub / connector chatter."""
    logging.basicConfig(level=logging.WARNING, format="%(levelname)s %(message)s")
    for noisy in ("httpx", "huggingface_hub", "snowflake.connector", "transformers"):
        logging.getLogger(noisy).setLevel(logging.ERROR)


def progress() -> Progress:
    """A consistent progress bar: description · bar · count · ETA."""
    return Progress(
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        MofNCompleteColumn(),
        TimeRemainingColumn(),
        console=console,
    )


def build_extractor(
    model: str,
    revision: str = "",
    *,
    constrained: bool = False,
    quantization: str = "",
):
    """Resolve a --model choice and load the transformers extractor.

    Prints the resolved id (and, for 'auto', the memory it was sized against)
    so a run records which checkpoint actually executed.
    """
    choice = resolve(model)
    if model == "auto":
        console.print(
            f"[dim]auto-selected {choice.model_id} for "
            f"~{available_memory_gb():.0f} GB — see `extract models`[/dim]"
        )
    kwargs: dict = {}
    if quantization:
        kwargs["quantization"] = quantization
    return TextExtractor(
        choice.model_id,
        revision=revision or None,
        constrained=constrained,
        **kwargs,
    )


def read(path: Path) -> pl.DataFrame:
    if path.suffix == ".parquet":
        return pl.read_parquet(path)
    if path.suffix in {".csv", ".tsv"}:
        return pl.read_csv(path, separator="\t" if path.suffix == ".tsv" else ",")
    msg = f"Unsupported input format: {path.suffix}"
    raise typer.BadParameter(msg)


def write(df: pl.DataFrame, path: Path) -> None:
    if path.suffix == ".parquet":
        df.write_parquet(path)
    elif path.suffix == ".csv":
        df.write_csv(path)
    else:
        msg = f"Unsupported output format: {path.suffix}"
        raise typer.BadParameter(msg)
