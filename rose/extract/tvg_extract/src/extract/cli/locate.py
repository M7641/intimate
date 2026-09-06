"""A4 — span extraction: locate the schema's fields, don't generate them."""

from __future__ import annotations

from pathlib import Path

import typer

from extract.cli.common import (
    configure_logging,
    console,
    progress,
    read,
    write,
)
from extract.domains import get_schema


def locate(
    domain: str,
    input_path: Path,
    output_path: Path,
    gliner_model: str = typer.Option(
        "urchade/gliner_medium-v2.1", help="GLiNER checkpoint (zero-shot NER)"
    ),
    threshold: float = typer.Option(
        0.4, help="minimum span confidence — lower finds more, noisier spans"
    ),
    batch_size: int = typer.Option(32, help="texts per GLiNER forward pass"),
    description_col: str = "description",
) -> None:
    """
    Span extraction (A4): locate the schema's fields in the text — don't
    generate them.

    GLiNER tags each DISTINCT description with the schema's fields as
    zero-shot entity types (one forward pass per product, no decoding loop);
    spans are validated through the schema like any extractor output, so the
    result is comparable to the LLM pipeline on the same gold set. Needs the
    optional `gliner` package (`uv pip install gliner`).

    Example:
        $ uv run extract locate clothing products.csv out.csv
    """
    from extract.spans import GLiNERLocator, locate_frame

    configure_logging()
    schema = get_schema(domain)
    df = read(input_path)
    console.print(f"[dim]{df.height} rows — loading GLiNER…[/dim]")
    locator = GLiNERLocator(gliner_model, threshold=threshold)
    with progress() as bar:
        task = bar.add_task("Locating spans", total=df.height)
        out = locate_frame(
            df,
            locator,
            schema,
            description_col=description_col,
            batch_size=batch_size,
            on_progress=lambda done, total: bar.update(
                task, completed=done, total=total
            ),
        )
    write(out, output_path)
    console.print(f"[green]Wrote[/green] {out.height} rows → {output_path}")
