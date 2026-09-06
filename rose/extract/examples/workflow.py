#!/usr/bin/env python3
"""End-to-end example workflow: dataset → extraction → printed results.

This is the library's "hello world" at catalogue scale. It mirrors what the
`extract run` CLI does, but laid out as explicit, narrated stages so you can see
the shape of a real job:

    1. Load the product dataset (polars).
    2. Build the extraction schema for a domain.
    3. Load the vision-language model once.
    4. Run extraction over every row (polars in, enriched polars out).
    5. Persist the enriched frame.
    6. Print the extracted features as a table.

Run it (downloads the model on first use):

    uv run python examples/workflow.py
    uv run python examples/workflow.py --domain clothing --input products.csv
    uv run python examples/workflow.py --list-models
    uv run python examples/workflow.py --model qwen2.5-vl-7b

The model defaults to `auto`: the largest Qwen2.5-VL checkpoint that fits this
machine's memory (see `--list-models`). Pass a registry key or a raw Hugging
Face id to override. The defaults point at the `products.csv` shipped at the
project root, so it runs with no arguments once a model is available.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import polars as pl
from rich.console import Console
from rich.table import Table

from extract import VisionLanguageExtractor, extract_features, get_schema
from extract.extractor import (
    MODELS,
    available_memory_gb,
    recommend_model,
    resolve,
)

PROJECT_ROOT = Path(__file__).resolve().parents[1]
console = Console()


def parse_args() -> argparse.Namespace:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--domain", default="electronics", help="extraction domain")
    ap.add_argument(
        "--input",
        type=Path,
        default=PROJECT_ROOT / "examples" / "products.csv",
        help="input CSV (id, description, image_path/image_url)",
    )
    ap.add_argument(
        "--output",
        type=Path,
        default=PROJECT_ROOT / "examples" / "out.csv",
        help="where to write the enriched frame (defaults beside the dataset)",
    )
    ap.add_argument(
        "--model",
        default="auto",
        help=(
            "model to use: 'auto' (largest that fits this machine), a registry "
            f"key ({', '.join(MODELS)}), or any Hugging Face VLM id"
        ),
    )
    ap.add_argument(
        "--list-models",
        action="store_true",
        help="print the available models for this machine and exit",
    )
    return ap.parse_args()


def stage(n: int, message: str) -> None:
    console.rule(f"[bold cyan]Stage {n}[/] · {message}")


def print_models() -> None:
    """Show the model registry, flagging which fit the detected memory."""
    mem = available_memory_gb()
    pick = recommend_model()
    console.print(f"Detected device memory: [bold]~{mem:.0f} GB[/]")
    table = Table("key", "params", "fits?", "Hugging Face id", "notes", show_lines=True)
    for key, m in MODELS.items():
        fits = "[green]✓[/]" if m.min_memory_gb <= mem else "[red]✗[/]"
        marker = " [bold cyan]← auto[/]" if key == pick else ""
        table.add_row(key + marker, m.params, fits, m.model_id, m.note)
    console.print(table)


def main() -> int:
    args = parse_args()

    if args.list_models:
        print_models()
        return 0

    choice = resolve(args.model)

    stage(1, f"Load dataset · {args.input}")
    df = pl.read_csv(args.input)
    # image_path entries are relative to the CSV; make them absolute so the run
    # works from any working directory (otherwise a relative path resolves
    # against the CWD and the local image is missed).
    if "image_path" in df.columns:
        base = args.input.resolve().parent.as_posix()
        df = df.with_columns(
            pl.when(
                (pl.col("image_path").str.len_chars() > 0)
                & ~pl.col("image_path").str.starts_with("/")
            )
            .then(pl.lit(base) + "/" + pl.col("image_path"))
            .otherwise(pl.col("image_path"))
            .alias("image_path")
        )
    console.print(f"  {df.height} products, columns: {df.columns}")

    stage(2, f"Build schema · domain={args.domain}")
    schema = get_schema(args.domain)
    console.print(f"  features: {', '.join(schema.column_names())}")

    stage(3, f"Load model · {choice.model_id}")
    if args.model == "auto":
        console.print(
            f"  auto-selected for ~{available_memory_gb():.0f} GB "
            f"(override with --model <key>; see --list-models)"
        )
    extractor = VisionLanguageExtractor(choice.model_id, loader=choice.loader)
    console.print(f"  device={extractor.device} dtype={extractor.dtype}")

    stage(4, "Extract features (one row at a time)")
    enriched = extract_features(df, schema, extractor)

    stage(5, f"Write output · {args.output}")
    enriched.write_csv(args.output)
    console.print(f"  wrote {enriched.height} rows")

    stage(6, "Results")
    _print_results(enriched, schema.column_names())
    return 0


def _print_results(df: pl.DataFrame, feature_cols: list[str]) -> None:
    """Render each product and its extracted features as a table."""
    present = [c for c in feature_cols if c in df.columns]
    table = Table("id", "description", *present, show_lines=True)
    for row in df.iter_rows(named=True):
        desc = str(row.get("description", ""))[:50]
        cells = [_fmt(row.get(c)) for c in present]
        table.add_row(str(row.get("id", "")), desc, *cells)
    console.print(table)


def _fmt(value: object) -> str:
    return "[dim]—[/]" if value is None else str(value)


if __name__ == "__main__":
    raise SystemExit(main())
