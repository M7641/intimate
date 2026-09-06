"""A2 — cluster raw descriptions into a categorical feature."""

from __future__ import annotations

import json
from pathlib import Path

import polars as pl
import typer
from rich.table import Table

from extract.cli.common import (
    DEFAULT_ENCODER_ID,
    ENCODER_BATCH_HELP,
    ENCODER_HELP,
    POOLING_HELP,
    build_extractor,
    configure_logging,
    console,
    progress,
    read,
    write,
)
from extract.domains import get_schema


def cluster(
    input_path: Path,
    output_path: Path,
    clusters_path: Path | None = typer.Option(
        None,
        "--clusters",
        help="write the per-cluster summary (.json): id, size, exemplar, terms "
        "— everything needed to name each cluster once",
    ),
    threshold: float = typer.Option(
        0.65, help="cosine merge threshold — lower = fewer, broader clusters"
    ),
    encoder: str = typer.Option(DEFAULT_ENCODER_ID, help=ENCODER_HELP),
    pooling: str = typer.Option("", help=POOLING_HELP),
    batch_size: int = typer.Option(64, help=ENCODER_BATCH_HELP),
    name_model: str = typer.Option(
        "",
        help="name each cluster with ONE LLM call on its exemplar (a model key "
        "or HF id, e.g. qwen2.5-0.5b); blank = no naming, use the exemplar/terms",
    ),
    description_col: str = "description",
) -> None:
    """
    Cluster raw descriptions into a categorical feature (A2) — no per-product
    generation.

    Embeds each DISTINCT description, greedy-clusters by cosine, and adds a
    `cluster_id` column. Each cluster carries its medoid exemplar and most
    distinctive terms, so naming happens once per cluster — by eye, or with
    --name-model (one LLM call per cluster, not per product).

    Example:
        $ uv run extract cluster products.csv out.csv --clusters clusters.json
        $ uv run extract cluster products.csv out.csv --name-model qwen2.5-0.5b
    """
    from extract.cluster import cluster_frame, name_clusters
    from extract.embeddings import Encoder

    configure_logging()
    df = read(input_path)
    console.print(f"[dim]{df.height} rows — loading encoder…[/dim]")
    enc = Encoder(encoder, pooling=pooling or None)
    with progress() as bar:
        task = bar.add_task("Clustering descriptions", total=df.height)
        out, clusters = cluster_frame(
            df,
            enc,
            description_col=description_col,
            threshold=threshold,
            batch_size=batch_size,
            on_progress=lambda done, total: bar.update(
                task, completed=done, total=total
            ),
        )
    console.print(
        f"[dim]{clusters.height} clusters over {df.height} rows "
        f"(threshold {threshold})[/dim]"
    )

    if name_model:
        schema = get_schema("generic")
        extractor = build_extractor(name_model)
        with progress() as bar:
            task = bar.add_task("Naming clusters", total=clusters.height)
            done = 0

            def label(exemplar: str) -> str | None:
                nonlocal done
                done += 1
                bar.update(task, completed=done)
                return extractor.extract(schema, exemplar).get("product_type")

            clusters = name_clusters(clusters, label)
        out = out.join(
            clusters.select("cluster_id", pl.col("name").alias("cluster_name")),
            on="cluster_id",
            how="left",
        )

    write(out, output_path)
    console.print(f"[green]Wrote[/green] {out.height} rows → {output_path}")
    if clusters_path is not None:
        clusters_path.write_text(
            json.dumps(clusters.to_dicts(), indent=2, ensure_ascii=False)
        )
        console.print(f"clusters → {clusters_path}")

    preview = Table("cluster", "size", "name" if name_model else "terms", "exemplar")
    for r in clusters.sort("size", descending=True).head(10).iter_rows(named=True):
        aid = r["name"] if name_model else ", ".join(r["terms"])
        preview.add_row(
            str(r["cluster_id"]), str(r["size"]), str(aid), r["exemplar"][:60]
        )
    console.print(preview)
