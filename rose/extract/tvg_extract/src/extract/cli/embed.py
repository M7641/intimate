"""A1 — embed product descriptions and store compressed vectors."""

from __future__ import annotations

import json
from pathlib import Path

import polars as pl
import typer

from extract.cli.common import (
    DEFAULT_ENCODER_ID,
    DEPARTMENT_HELP,
    DIVISION_HELP,
    ENCODER_BATCH_HELP,
    ENCODER_HELP,
    POOLING_HELP,
    configure_logging,
    console,
    progress,
    write,
)


def embed(
    sample: int = typer.Option(500, help="products to sample from the warehouse"),
    all_in_category: bool = typer.Option(
        False,
        "--all",
        help="embed EVERY new product in the chosen category instead of a "
        "--sample of it; requires --division or --department so a run can never "
        "sweep the whole warehouse by accident",
    ),
    division: str = typer.Option("", help=DIVISION_HELP),
    department: str = typer.Option("", help=DEPARTMENT_HELP),
    encoder: str = typer.Option(
        DEFAULT_ENCODER_ID,
        help=ENCODER_HELP + " Switching encoders requires clearing the "
        "embedding table (vectors from different encoders are not comparable).",
    ),
    pooling: str = typer.Option("", help=POOLING_HELP),
    dim: int = typer.Option(
        16,
        help="compressed embedding width — PCA-projected from the encoder's "
        "native width; the projection is fitted once and persisted so every "
        "run lands in the same space",
    ),
    batch_size: int = typer.Option(64, help=ENCODER_BATCH_HELP),
    output: Path | None = typer.Option(
        None, help="optional local snapshot (.csv/.parquet)"
    ),
) -> None:
    """
    Embed product descriptions and store compressed vectors (A1).

    Samples new products (anti-join on the embedding table), encodes each
    DISTINCT description once with the sentence encoder, compresses to --dim
    via a persisted PCA projection (fitted on the first run, reused after),
    and writes id + vector to the warehouse. No generation anywhere.

    The first run fits the projection, so give it a representative sample
    (at least --dim distinct descriptions; a few thousand is better).

    Pass --all to embed an ENTIRE category in one sweep (every new product in
    the --division/--department) rather than a --sample of it. The run is
    resumable: the warehouse anti-join skips ids already stored, so a re-run
    after an interruption only picks up the remainder.

    Examples:
        $ uv run extract embed --sample 2000 --department FURNITURE --dim 16
        $ uv run extract embed --all --department FURNITURE --dim 16
    """
    # Snowflake-only deps imported lazily, like `hierarchy`.
    from extract.database import DBActions
    from extract.datasource import load_products
    from extract.embeddings import Encoder, embed_frame
    from extract.output import (
        init_embedding_table,
        init_projection_table,
        load_projection,
        save_projection,
        stored_embedding_models,
        write_embeddings,
    )

    if all_in_category and not (division or department):
        raise typer.BadParameter(
            "--all needs a category to bound the run: pass --division or "
            "--department. Without one it would embed the whole warehouse."
        )

    configure_logging()
    db = DBActions()
    try:
        init_embedding_table(db)
        init_projection_table(db)
        stored = stored_embedding_models(db)
        if stored and stored != [encoder]:
            raise typer.BadParameter(
                f"the embedding table already holds vectors from {stored}; "
                f"vectors from different encoders are not comparable, and the "
                f"id anti-join would never re-embed the old rows. Clear the "
                f"table first (`extract init --recreate`) to switch to "
                f"'{encoder}'."
            )
        scope = "whole category" if all_in_category else f"sample of {sample}"
        with console.status(f"Loading new products from the warehouse ({scope})…"):
            df = load_products(
                sample_size=None if all_in_category else sample,
                exclude_schema="sandpit",
                exclude_table="llm_product_embedding_proto",
                division=division or None,
                department=department or None,
            )
        console.print(f"[dim]{df.height} new products[/dim]")
        if df.is_empty():
            console.print("nothing new — done")
            return

        with console.status("Loading encoder…"):
            enc = Encoder(encoder, pooling=pooling or None)
        console.print(f"[dim]encoder={encoder} pooling={enc.pooling}[/dim]")
        projection = load_projection(db, encoder, dim)
        console.print(
            f"[dim]projection: {'reusing stored' if projection else 'fitting on this sample'} "
            f"(→ {dim} dims)[/dim]"
        )

        with progress() as bar:
            task = bar.add_task("Embedding descriptions", total=df.height)
            out, fitted = embed_frame(
                df,
                enc,
                dim=dim,
                projection=projection,
                batch_size=batch_size,
                on_progress=lambda done, total: bar.update(
                    task, completed=done, total=total
                ),
            )
        if projection is None:
            save_projection(db, fitted, encoder)
            console.print("[dim]stored the fitted projection[/dim]")

        with console.status("Storing embeddings…"):
            written = write_embeddings(db, out, model_id=encoder, dim=dim)
        console.print(f"[green]Wrote[/green] {written} embeddings to the warehouse")

        if output is not None:
            # CSV cannot hold a list column; stringify the vector for snapshots.
            snapshot = out.with_columns(
                pl.col("embedding").map_elements(json.dumps, return_dtype=pl.Utf8)
            )
            write(snapshot, output)
            console.print(f"snapshot → {output}")
    finally:
        db.close()
