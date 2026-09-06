"""The two-stage warehouse pipeline: init, hierarchy (Stage 1), condense
(Stage 2), discover.

Snowflake-only dependencies are imported inside each command, so the rest of
the CLI stays usable without a warehouse connection.
"""

from __future__ import annotations

import json
from pathlib import Path

import polars as pl
import typer
from rich.table import Table

from extract.cli.common import (
    BATCH_HELP,
    CONSTRAINED_HELP,
    DEDUP_HELP,
    DEPARTMENT_HELP,
    DIVISION_HELP,
    GROUP_HELP,
    MODEL_HELP,
    NEAR_HELP,
    QUANTIZATION_HELP,
    WAREHOUSE_CACHE_HELP,
    build_extractor,
    configure_logging,
    console,
    progress,
    read,
    write,
)
from extract.domains import get_schema
from extract.pipeline import extract_features


def init(
    recreate: bool = typer.Option(
        False,
        help="DROP + recreate the output tables (clears data) — use after a schema change",
    ),
    log_query: bool = typer.Option(False, help="echo the rendered DDL"),
) -> None:
    """
    Create the output tables (L1 raw, L2 concepts, L3 final, caches, embeddings).

    `create table if not exists` can't migrate an existing table, so after
    changing the generated schema (e.g. → product_type) run with --recreate to
    drop and rebuild them.

    Example:
        $ uv run extract init --recreate
    """
    from extract.database import DBActions
    from extract.output import init_all

    db = DBActions()
    try:
        init_all(db, recreate=recreate, log_query=log_query)
    finally:
        db.close()
    console.print(
        f"[green]{'Recreated' if recreate else 'Ready'}[/green] output tables"
    )


def hierarchy(
    sample: int = typer.Option(50, help="products to sample from the warehouse"),
    division: str = typer.Option("", help=DIVISION_HELP),
    department: str = typer.Option("", help=DEPARTMENT_HELP),
    model: str = typer.Option("qwen2.5-1.5b", help=MODEL_HELP),
    batch_size: int = typer.Option(4, help=BATCH_HELP),
    group_size: int = typer.Option(1, help=GROUP_HELP),
    constrained: bool = typer.Option(False, help=CONSTRAINED_HELP),
    quantization: str = typer.Option("", help=QUANTIZATION_HELP),
    dedup: bool = typer.Option(True, help=DEDUP_HELP),
    near_threshold: float = typer.Option(0.0, help=NEAR_HELP),
    cache: bool = typer.Option(False, help=WAREHOUSE_CACHE_HELP),
    output: Path | None = typer.Option(
        None, help="optional local snapshot (.csv/.parquet)"
    ),
) -> None:
    """
    Generate a product_type from the warehouse and store it (Stage 1).

    Samples products, skips ids already stored (anti-join on id), generates
    the `product_type` with the `generic` schema, and writes the new rows to
    the output table. Run `extract init` once first to create the table.

    Example:
        $ uv run extract hierarchy --sample 1000 --division Womenswear
        $ uv run extract hierarchy --sample 1000 --department FURNITURE
    """
    from extract.database import DBActions
    from extract.datasource import load_products
    from extract.output import init_table, write_hierarchy

    configure_logging()
    schema = get_schema("generic")
    db = DBActions()
    try:
        init_table(db)  # idempotent: create the output table if absent
        with console.status("Loading new products from the warehouse…"):
            # Anti-join already-stored ids in-warehouse, so the sample is all new.
            df = load_products(
                sample_size=sample,
                exclude_schema="sandpit",
                exclude_table="llm_product_hierarchy_proto",
                division=division or None,
                department=department or None,
            )
        console.print(f"[dim]{df.height} new products[/dim]")
        if df.is_empty():
            console.print("nothing new — done")
            return

        extractor = build_extractor(
            model, constrained=constrained, quantization=quantization
        )
        console.print(f"[dim]device={extractor.device} dtype={extractor.dtype}[/dim]")
        extraction_cache = None
        if cache:
            # The warehouse outlives the (ephemeral) workflow container; reuse
            # the connection the command already holds.
            from extract.output import WarehouseExtractionCache

            extraction_cache = WarehouseExtractionCache(
                db, model_id=extractor.model_id, schema_domain=schema.domain
            )
            console.print(
                f"[dim]warehouse cache: {len(extraction_cache.entries)} entries[/dim]"
            )
        if group_size > 1:
            mode = f"{group_size} descriptions per prompt"
        elif batch_size == 0:
            mode = "the whole sample in one call"
        elif batch_size > 1:
            mode = f"batches of {batch_size}"
        else:
            mode = "one row at a time"
        console.print(f"[dim]generating {mode}[/dim]")
        with progress() as bar:
            # Dedup can shrink the work below df.height; trust the callback's
            # total (the number of representatives actually sent to the model).
            task = bar.add_task("Generating hierarchy", total=df.height)
            out = extract_features(
                df,
                schema,
                extractor,
                batch_size=batch_size,
                group_size=group_size,
                dedup=dedup,
                near_threshold=near_threshold or None,
                cache=extraction_cache,
                on_progress=lambda done, total: bar.update(
                    task, completed=done, total=total
                ),
            )

        with console.status("Storing rows…"):
            written = write_hierarchy(
                db, out, model_id=extractor.model_id, schema_domain=schema.domain
            )
        console.print(f"[green]Wrote[/green] {written} rows to the output table")
        if output is not None:
            write(out, output)
            console.print(f"snapshot → {output}")

        table = Table("id", "description", *schema.column_names(), show_lines=True)
        for row in out.iter_rows(named=True):
            desc = str(row.get("description", "")).replace("\n", " ")[:50]
            cells = [
                "—" if row.get(c) is None else str(row.get(c))
                for c in schema.column_names()
            ]
            table.add_row(str(row.get("id", "")), desc, *cells)
        console.print(table)
    finally:
        db.close()


def condense(
    threshold: float = typer.Option(
        0.65, help="cosine match threshold — looser = fewer, broader concepts"
    ),
    min_support: int = typer.Option(
        3, help="occurrences before a new concept is promoted (the gate)"
    ),
) -> None:
    """
    Condense the raw free-text product_type into a concept taxonomy (Stage 2).

    Incremental: processes only raw rows not yet in the final table, embeds just
    their distinct new values, and assigns them against stored concept centroids
    — adding a concept only when a value recurs `min_support` times without
    fitting. Run `extract hierarchy` first to populate the raw table.

    Example:
        $ uv run extract condense --min-support 3
    """
    configure_logging()
    from extract.condense import (
        LEVELS,
        UNALLOCATED,
        condense_batch,
        distinct_values,
        embed_values,
        reconcile_batch,
    )
    from extract.database import DBActions
    from extract.output import (
        init_concept_table,
        init_final_table,
        load_concepts,
        load_unallocated,
        load_unmapped,
        save_concepts,
        update_final,
        write_final,
    )

    db = DBActions()
    try:
        init_concept_table(db)
        init_final_table(db)
        with console.status("Loading new + parked rows…"):
            new_rows = load_unmapped(db).to_dicts()
            parked = load_unallocated(db)
        if not new_rows and not parked:
            console.print("nothing to condense — run `extract hierarchy` first")
            return

        # Embed the whole working set once (new batch + parked raw text).
        values = distinct_values(new_rows + parked)
        console.print(f"[dim]{len(new_rows)} new, {len(parked)} parked[/dim]")
        with console.status(f"Embedding {len(values)} distinct values…"):
            vectors = embed_values(values)
        concepts = load_concepts(db)
        stats = {
            "new_concepts": 0,
            "promoted": 0,
            "active": sum(1 for c in concepts if c["status"] == "active"),
            "pending": sum(1 for c in concepts if c["status"] == "pending"),
        }

        written, replaced, still_parked = 0, 0, 0
        with console.status("Condensing + writing…"):
            if new_rows:
                concepts, mapped, stats = condense_batch(
                    concepts,
                    new_rows,
                    vectors,
                    threshold=threshold,
                    min_support=min_support,
                )
                save_concepts(db, concepts)
                written = write_final(db, mapped)

            # Reconcile: re-place parked rows against the (now larger) active tree.
            if parked:
                remapped = reconcile_batch(
                    concepts, parked, vectors, threshold=threshold
                )
                changed = [
                    m
                    for m, p in zip(remapped, parked)
                    if tuple(m[lvl] for lvl in LEVELS)
                    != tuple(p[f"cur_{lvl}"] for lvl in LEVELS)
                ]
                replaced = update_final(db, changed)
                still_parked = sum(
                    1 for m in remapped if any(m[lvl] == UNALLOCATED for lvl in LEVELS)
                )
    finally:
        db.close()

    console.print(
        f"[green]Condensed[/green] {written} new + {replaced} reconciled rows · "
        f"+{stats['new_concepts']} concepts, {stats['promoted']} promoted · "
        f"{stats['active']} active / {stats['pending']} pending"
    )
    table = Table("level", "active", "pending")
    for level in LEVELS:
        table.add_row(
            level,
            str(
                sum(
                    1
                    for c in concepts
                    if c["level"] == level and c["status"] == "active"
                )
            ),
            str(
                sum(
                    1
                    for c in concepts
                    if c["level"] == level and c["status"] == "pending"
                )
            ),
        )
    console.print(table)
    if still_parked:
        console.print(
            f"[yellow]{still_parked}[/] rows still unallocated (no matching active concept yet)"
        )


def discover(
    input: Path | None = typer.Option(
        None,
        help="generated values file (.csv/.parquet); default: read the output table",
    ),
    threshold: float = typer.Option(
        0.65,
        help="cosine merge threshold — the concentration dial (lower = fewer clusters)",
    ),
    output: Path | None = typer.Option(
        None, help="write the discovered taxonomy (.json)"
    ),
) -> None:
    """
    Discover a categorical taxonomy from generated free-text values.

    Clusters product_type into canonical labels and reports the
    resulting concentration (cardinality / entropy). The `discover` half of
    discover-then-constrain: freeze the emitted choices into a CATEGORICAL
    schema — or feed the file straight to `extract classify --labels`.

    Example:
        $ uv run extract discover --threshold 0.6 --output taxonomy.json
    """
    configure_logging()
    from extract.discover import concentration, discover_taxonomy

    if input is not None:
        df = read(input)
    else:
        from extract.database import DBActions
        from extract.output import load_hierarchy

        db = DBActions()
        try:
            df = load_hierarchy(db)
        finally:
            db.close()

    with console.status("Embedding + clustering values…"):
        taxonomy = discover_taxonomy(df, threshold=threshold)
    if taxonomy.is_empty():
        console.print("no values to cluster — run `extract hierarchy` first")
        return

    metrics = Table("level", "raw values", "clusters", "norm entropy", "top share")
    for r in concentration(taxonomy).iter_rows(named=True):
        metrics.add_row(
            r["level"],
            str(r["raw_values"]),
            str(r["clusters"]),
            f"{r['norm_entropy']:.2f}",
            f"{r['top_share']:.0%}",
        )
    console.print(metrics)

    levels = ("product_type",)
    for level in levels:
        group = taxonomy.filter(pl.col("level") == level).sort(
            "support", descending=True
        )
        if group.is_empty():
            continue
        console.print(f"\n[bold]{level}[/] — {group.height} canonical labels")
        top = Table("canonical", "support", "absorbed")
        for r in group.head(10).iter_rows(named=True):
            top.add_row(r["canonical"], str(r["support"]), ", ".join(r["members"][:6]))
        console.print(top)

    if output is not None:
        payload = {}
        for level in levels:
            group = taxonomy.filter(pl.col("level") == level).sort(
                "support", descending=True
            )
            if group.is_empty():
                continue
            payload[level] = {
                "choices": group.get_column("canonical").to_list(),
                "clusters": [
                    {
                        "canonical": r["canonical"],
                        "support": r["support"],
                        "members": r["members"],
                    }
                    for r in group.iter_rows(named=True)
                ],
            }
        output.write_text(json.dumps(payload, indent=2, ensure_ascii=False))
        console.print(f"\n[green]Wrote[/green] taxonomy → {output}")
