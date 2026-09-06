"""The LLM extraction path over local files: one, run, eval."""

from __future__ import annotations

import datetime
import json
from pathlib import Path

import typer
from rich.table import Table

from extract.cli.common import (
    BATCH_HELP,
    CACHE_HELP,
    CONSTRAINED_HELP,
    DEDUP_HELP,
    GROUP_HELP,
    MODEL_HELP,
    NEAR_HELP,
    QUANTIZATION_HELP,
    build_extractor,
    configure_logging,
    console,
    read,
    write,
)
from extract.domains import get_schema
from extract.evaluation import evaluate
from extract.pipeline import extract_features, stamp_provenance


def build_cache(cache_path: Path | None, extractor, schema):
    """A local JSONL ExtractionCache bound to this run's model + schema, or None."""
    if cache_path is None:
        return None
    from extract.dedup import ExtractionCache

    return ExtractionCache(
        cache_path, model_id=extractor.model_id, schema_domain=schema.domain
    )


def one(
    domain: str,
    description: str,
    model: str = typer.Option("auto", help=MODEL_HELP),
    constrained: bool = typer.Option(False, help=CONSTRAINED_HELP),
    quantization: str = typer.Option("", help=QUANTIZATION_HELP),
    revision: str = "",
) -> None:
    """
    Extract features for a single product description and print the JSON.

    Example:
        $ uv run extract one clothing "Red cotton t-shirt, crew neck"
    """
    schema = get_schema(domain)
    extractor = build_extractor(
        model, revision, constrained=constrained, quantization=quantization
    )
    console.print(f"[dim]device={extractor.device} dtype={extractor.dtype}[/dim]")
    result = extractor.extract(schema, description)
    console.print_json(json.dumps(result, ensure_ascii=False))


def run(
    domain: str,
    input_path: Path,
    output_path: Path,
    model: str = typer.Option("auto", help=MODEL_HELP),
    constrained: bool = typer.Option(False, help=CONSTRAINED_HELP),
    quantization: str = typer.Option("", help=QUANTIZATION_HELP),
    revision: str = "",
    batch_size: int = typer.Option(1, help=BATCH_HELP),
    group_size: int = typer.Option(1, help=GROUP_HELP),
    dedup: bool = typer.Option(True, help=DEDUP_HELP),
    near_threshold: float = typer.Option(0.0, help=NEAR_HELP),
    cache: Path | None = typer.Option(None, help=CACHE_HELP),
    provenance: bool = True,
    description_col: str = "description",
) -> None:
    """
    Enrich a product file (csv/parquet) with extracted features.

    Each row is extracted from its ``description`` text. Use --batch-size N
    (0 = everything in one call) or --group-size N for throughput, --revision
    to pin the model commit. Distinct descriptions are extracted once and
    broadcast (--no-dedup to disable); --cache persists results across runs.

    Example:
        $ uv run extract run clothing products.csv out.csv --batch-size 8
    """
    # Surface the pipeline's per-row failure warnings and final summary.
    configure_logging()
    schema = get_schema(domain)
    df = read(input_path)
    console.print(f"[dim]{df.height} rows — loading model…[/dim]")
    extractor = build_extractor(
        model, revision, constrained=constrained, quantization=quantization
    )
    out = extract_features(
        df,
        schema,
        extractor,
        description_col=description_col,
        batch_size=batch_size,
        group_size=group_size,
        dedup=dedup,
        near_threshold=near_threshold or None,
        cache=build_cache(cache, extractor, schema),
    )
    if provenance:
        # Record the resolved checkpoint, not the literal "auto"/key the user typed.
        pinned = f"{extractor.model_id}@{revision}" if revision else extractor.model_id
        now = datetime.datetime.now(datetime.timezone.utc).isoformat()
        out = stamp_provenance(
            out, model_id=pinned, schema_domain=schema.domain, at=now
        )
    write(out, output_path)
    console.print(f"[green]Wrote[/green] {out.height} rows → {output_path}")


def evaluate_cmd(
    domain: str,
    gold_path: Path,
    model: str = typer.Option("auto", help=MODEL_HELP),
    constrained: bool = typer.Option(False, help=CONSTRAINED_HELP),
    quantization: str = typer.Option("", help=QUANTIZATION_HELP),
) -> None:
    """
    Score extraction against a hand-labelled gold set (per-field accuracy).

    The gold file is an input file (id, description) plus one column
    per schema feature holding the expected value; blank cells are unlabelled.

    Example:
        $ uv run extract eval clothing path/to/gold.csv
    """
    configure_logging()
    schema = get_schema(domain)
    gold = read(gold_path)
    console.print(f"[dim]{gold.height} gold rows — loading model…[/dim]")
    extractor = build_extractor(
        model, constrained=constrained, quantization=quantization
    )
    pred, report = evaluate(gold, schema, extractor)

    table = Table("field", "support", "correct", "accuracy")
    for score in report.per_field:
        acc = "—" if score.accuracy is None else f"{score.accuracy:.0%}"
        table.add_row(score.field, str(score.support), str(score.correct), acc)
    console.print(table)
    micro = report.micro_accuracy
    macro = report.macro_accuracy
    console.print(
        f"[bold]micro[/] {micro:.1%}  ·  [bold]macro[/] {macro:.1%}"
        if micro is not None
        else "[yellow]no labelled cells to score[/]"
    )
