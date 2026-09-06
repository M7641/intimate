"""Command-line interface — assembly only.

The commands live in one module per pattern (see the package layout); this
module registers them onto the typer app in the order the help should list
them. Command functions are plain functions (no decorators), so each module
stays importable and testable on its own.

    extract describe clothing
    extract one electronics "ASUS ROG B650-E gaming motherboard"
    extract run clothing products.csv out.parquet
"""

from __future__ import annotations

import typer

from extract.cli import (
    classify,
    cluster,
    deploy,
    embed,
    info,
    llm,
    locate,
    warehouse,
)

app = typer.Typer(
    add_completion=False,
    help="Product feature extraction: an LLM pipeline plus cheaper "
    "non-generative paths (embed/cluster/classify/locate).",
)

# The two-stage warehouse pipeline.
app.command()(warehouse.init)
app.command()(warehouse.hierarchy)
app.command()(warehouse.condense)
app.command()(warehouse.discover)

# Section A — the non-generative paths.
app.command()(embed.embed)
app.command()(cluster.cluster)
app.command()(classify.classify)
app.command()(locate.locate)

# The LLM path over local files.
app.command()(llm.run)
app.command()(llm.one)
app.command(name="eval")(llm.evaluate_cmd)

# Ops and introspection.
app.command()(deploy.deploy)
app.command()(info.domains)
app.command()(info.models)
app.command()(info.describe)


if __name__ == "__main__":
    app()
