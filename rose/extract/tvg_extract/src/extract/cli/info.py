"""Introspection commands — no model, no warehouse: domains, models, describe."""

from __future__ import annotations

from rich.table import Table

from extract.cli.common import console
from extract.domains import SCHEMAS, get_schema
from extract.extractor import (
    MODELS,
    available_memory_gb,
    recommend_model,
)


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


def describe(domain: str) -> None:
    """
    Print the extraction contract (prompt) for a domain.

    Example:
        $ uv run extract describe clothing
    """
    console.print(get_schema(domain).prompt_block())
