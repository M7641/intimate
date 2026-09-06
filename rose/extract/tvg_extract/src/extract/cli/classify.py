"""A3 — zero-shot classification by retrieval."""

from __future__ import annotations

from pathlib import Path

import polars as pl
import typer

from extract.cli.common import (
    DEFAULT_ENCODER_ID,
    ENCODER_BATCH_HELP,
    ENCODER_HELP,
    POOLING_HELP,
    configure_logging,
    console,
    progress,
    read,
    write,
)


def classify(
    input_path: Path,
    output_path: Path,
    labels_path: Path = typer.Option(
        ...,
        "--labels",
        help="candidate labels (.json): a list of names, a list of "
        '{"name", "gloss"} objects (glosses embed better), or an '
        "`extract discover` taxonomy file",
    ),
    encoder: str = typer.Option(DEFAULT_ENCODER_ID, help=ENCODER_HELP),
    pooling: str = typer.Option("", help=POOLING_HELP),
    batch_size: int = typer.Option(64, help=ENCODER_BATCH_HELP),
    output_col: str = typer.Option("product_type", help="name of the label column"),
    description_col: str = "description",
) -> None:
    """
    Zero-shot classification by retrieval (A3) — argmax over label embeddings.

    Embeds every label once and each DISTINCT description once, assigns by
    max cosine. Adds the label plus `similarity` and `margin` columns — the
    margin (best minus runner-up) is the confidence signal a routing cascade
    (D1) thresholds on.

    Example:
        $ uv run extract classify products.csv out.csv --labels taxonomy.json
    """
    from extract.embeddings import Encoder
    from extract.retrieval import classify_frame, load_labels

    configure_logging()
    labels = load_labels(labels_path)
    df = read(input_path)
    console.print(
        f"[dim]{df.height} rows, {len(labels)} candidate labels — loading encoder…[/dim]"
    )
    enc = Encoder(encoder, pooling=pooling or None)
    with progress() as bar:
        task = bar.add_task("Classifying descriptions", total=df.height)
        out = classify_frame(
            df,
            enc,
            labels,
            description_col=description_col,
            output_col=output_col,
            batch_size=batch_size,
            on_progress=lambda done, total: bar.update(
                task, completed=done, total=total
            ),
        )
    write(out, output_path)
    low = out.filter(pl.col("margin") < 0.05).height
    console.print(
        f"[green]Wrote[/green] {out.height} rows → {output_path} "
        f"[dim]({low} rows with margin < 0.05 — D1 fallback candidates)[/dim]"
    )
