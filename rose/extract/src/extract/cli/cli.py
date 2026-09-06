"""Command-line interface.

extract describe clothing
extract one electronics ./mobo.jpg "ASUS ROG B650-E gaming motherboard"
extract run clothing products.csv out.parquet
"""

from __future__ import annotations

import datetime
import json
import logging
from pathlib import Path

import polars as pl
import typer
from rich.console import Console
from rich.table import Table

from extract.domains import SCHEMAS, get_schema
from extract.evaluation import evaluate
from extract.extractor import (
    MODELS,
    VisionLanguageExtractor,
    available_memory_gb,
    recommend_model,
    resolve,
)
from extract.pipeline import extract_features, stamp_provenance

app = typer.Typer(add_completion=False, help="Product feature extraction via a VLM.")
console = Console()

# Help string shared by every command's --model option.
_MODEL_HELP = (
    "model to use: 'auto' (largest registry model that fits this machine), a "
    f"registry key ({', '.join(MODELS)}), or any Hugging Face VLM id. "
    "See `extract models`."
)


def _build_extractor(model: str, revision: str = "") -> VisionLanguageExtractor:
    """Resolve a --model choice and load it with the right loader.

    Prints the resolved id (and, for 'auto', the memory it was sized against)
    so a run records which checkpoint actually executed.
    """
    choice = resolve(model)
    if model == "auto":
        console.print(
            f"[dim]auto-selected {choice.model_id} for "
            f"~{available_memory_gb():.0f} GB — see `extract models`[/dim]"
        )
    return VisionLanguageExtractor(
        choice.model_id, revision=revision or None, loader=choice.loader
    )


@app.command()
def domains() -> None:
    """
    List the available domains.

    Example:
        $ uv run extract domains
    """
    table = Table("domain", "description", "features")
    for name, schema in sorted(SCHEMAS.items()):
        table.add_row(name, schema.description, str(len(schema.column_names())))
    console.print(table)


@app.command()
def models() -> None:
    """
    List the available models, flagging which fit this machine's memory.

    Example:
        $ uv run extract models
    """
    mem = available_memory_gb()
    pick = recommend_model()
    console.print(f"Detected device memory: [bold]~{mem:.0f} GB[/]")
    table = Table("key", "params", "fits?", "Hugging Face id", "notes", show_lines=True)
    for key, m in MODELS.items():
        fits = "[green]✓[/]" if m.min_memory_gb <= mem else "[red]✗[/]"
        marker = " [bold cyan]← auto[/]" if key == pick else ""
        table.add_row(key + marker, m.params, fits, m.model_id, m.note)
    console.print(table)


@app.command()
def describe(domain: str) -> None:
    """
    Print the extraction contract (prompt) for a domain.

    Example:
        $ uv run extract describe clothing
    """
    console.print(get_schema(domain).prompt_block())


@app.command()
def one(
    domain: str,
    image: str,
    description: str = "",
    model: str = typer.Option("auto", help=_MODEL_HELP),
    revision: str = "",
) -> None:
    """
    Extract features for a single product and print the JSON.

    Example:
        $ uv run extract one clothing ./product.jpg "Red t-shirt"
    """
    schema = get_schema(domain)
    extractor = _build_extractor(model, revision)
    console.print(f"[dim]device={extractor.device} dtype={extractor.dtype}[/dim]")
    result = extractor.extract(schema, description, image)
    console.print_json(json.dumps(result, ensure_ascii=False))


@app.command()
def run(
    domain: str,
    input_path: Path,
    output_path: Path,
    model: str = typer.Option("auto", help=_MODEL_HELP),
    revision: str = "",
    batch_size: int = 1,
    provenance: bool = True,
    description_col: str = "description",
    image_col: str = "image_path",
    image_url_col: str = "image_url",
) -> None:
    """
    Enrich a product file (csv/parquet/vortex) with extracted features.

    Works directly on the CSV from `extract-scrape`: each row uses its local
    image_path if present, otherwise falls back to the remote image_url.
    Use --batch-size N for throughput, --revision to pin the model commit.

    Example:
        $ uv run extract run clothing products.csv out.csv --batch-size 8
    """
    # Surface the pipeline's per-row failure warnings and final summary.
    logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
    schema = get_schema(domain)
    df = _read(input_path)
    console.print(f"[dim]{df.height} rows — loading model…[/dim]")
    extractor = _build_extractor(model, revision)
    out = extract_features(
        df,
        schema,
        extractor,
        description_col=description_col,
        image_col=image_col,
        image_url_col=image_url_col,
        batch_size=batch_size,
    )
    if provenance:
        # Record the resolved checkpoint, not the literal "auto"/key the user typed.
        pinned = f"{extractor.model_id}@{revision}" if revision else extractor.model_id
        now = datetime.datetime.now(datetime.timezone.utc).isoformat()
        out = stamp_provenance(
            out, model_id=pinned, schema_domain=schema.domain, at=now
        )
    _write(out, output_path)
    console.print(f"[green]Wrote[/green] {out.height} rows → {output_path}")


@app.command(name="eval")
def evaluate_cmd(
    domain: str,
    gold_path: Path,
    model: str = typer.Option("auto", help=_MODEL_HELP),
) -> None:
    """
    Score extraction against a hand-labelled gold set (per-field accuracy).

    The gold file is an input file (id, description, [image]) plus one column
    per schema feature holding the expected value; blank cells are unlabelled.

    Example:
        $ uv run extract eval clothing examples/gold/clothing.csv
    """
    logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
    schema = get_schema(domain)
    gold = _read(gold_path)
    console.print(f"[dim]{gold.height} gold rows — loading model…[/dim]")
    extractor = _build_extractor(model)
    _pred, report = evaluate(gold, schema, extractor)

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


def _read(path: Path) -> pl.DataFrame:
    if path.suffix == ".parquet":
        return pl.read_parquet(path)
    if path.suffix == ".vortex":
        import vortex as vx  # heavy; only needed for .vortex

        return vx.open(path).to_polars().collect()
    if path.suffix in {".csv", ".tsv"}:
        return pl.read_csv(path, separator="\t" if path.suffix == ".tsv" else ",")
    msg = f"Unsupported input format: {path.suffix}"
    raise typer.BadParameter(msg)


def _write(df: pl.DataFrame, path: Path) -> None:
    if path.suffix == ".parquet":
        df.write_parquet(path)
    elif path.suffix == ".vortex":
        import vortex as vx  # heavy; only needed for .vortex

        vx.io.write(df.to_arrow(), str(path))
    elif path.suffix == ".csv":
        df.write_csv(path)
    else:
        msg = f"Unsupported output format: {path.suffix}"
        raise typer.BadParameter(msg)


if __name__ == "__main__":
    app()
