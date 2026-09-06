"""Batch orchestration: polars in, enriched polars out."""

from __future__ import annotations

import logging
from collections.abc import Callable, Iterable
from typing import Literal

import polars as pl

from extract.dedup import (
    ExtractionCache,
    description_key,
    merge_near_duplicates,
)
from extract.extractor import TextExtractor
from extract.schema import ExtractionSchema

logger = logging.getLogger(__name__)


def extract_features(
    df: pl.DataFrame,
    schema: ExtractionSchema,
    extractor: TextExtractor,
    *,
    description_col: str = "description",
    on_error: Literal["null", "raise"] = "null",
    batch_size: int = 1,
    group_size: int = 1,
    dedup: bool = True,
    near_threshold: float | None = None,
    embed: Callable[[list[str]], object] | None = None,
    cache: ExtractionCache | None = None,
    on_progress: Callable[[int, int], None] | None = None,
) -> pl.DataFrame:
    """Enrich ``df`` with one column per schema feature.

    Each row is extracted from its ``description`` text.

    ``on_error``: "null" fills a failed row with null values (and logs a warning
    with the reason), "raise" propagates the first failure.

    ``batch_size`` > 1 runs the model on whole batches via
    ``extractor.extract_batch`` — the main throughput lever. It trades per-row
    error isolation for speed: a failed batch nulls its rows (logged).
    ``batch_size=0`` sends the whole frame in ONE call.

    ``group_size`` > 1 instead packs that many descriptions into ONE prompt via
    ``extractor.extract_grouped`` (C3 prompt batching) — the schema contract is
    paid once per group, and a misaligned reply falls back to per-row inside
    the extractor. Mutually exclusive with ``batch_size`` != 1: the two levers
    batch at different layers (engine vs prompt).

    ``dedup`` (Step 0 in README, on by default) extracts each distinct
    description ONCE and broadcasts the result to its duplicate rows — same
    output, less compute. ``near_threshold`` additionally merges
    near-duplicates by embedding cosine similarity (a member inherits its
    group leader's features — an approximation, so opt-in). ``cache`` persists
    results across runs, keyed per description; cached descriptions skip the
    model entirely.
    """
    if on_error not in ("null", "raise"):
        msg = f"on_error must be 'null' or 'raise', got {on_error!r}"
        raise ValueError(msg)
    if description_col not in df.columns:
        msg = f"Expected column missing: {description_col!r}"
        raise ValueError(msg)
    if batch_size < 0 or group_size < 1:
        msg = f"batch_size must be >= 0 and group_size >= 1, got {batch_size}/{group_size}"
        raise ValueError(msg)
    if group_size > 1 and batch_size != 1:
        msg = "group_size and batch_size are mutually exclusive; set one of them"
        raise ValueError(msg)

    rows = list(df.iter_rows(named=True))
    if dedup or near_threshold is not None or cache is not None:
        records, failures = run_deduped(
            rows,
            schema,
            extractor,
            description_col,
            on_error,
            batch_size,
            group_size,
            near_threshold,
            embed,
            cache,
            on_progress,
        )
    else:
        records, failures = run_rows(
            rows,
            schema,
            extractor,
            description_col,
            on_error,
            batch_size,
            group_size,
            on_progress,
        )

    if failures:
        logger.warning("%d/%d rows failed extraction", failures, len(records))

    feats_df = records_to_frame(records, schema.column_names())
    return pl.concat([df, feats_df], how="horizontal")


def run_rows(
    rows,
    schema,
    extractor,
    description_col,
    on_error,
    batch_size,
    group_size,
    on_progress=None,
) -> tuple[list[dict], int]:
    """Dispatch a row list to the per-row, engine-batched or grouped path."""
    if group_size > 1 and hasattr(extractor, "extract_grouped"):
        logger.info("Running extraction with %d descriptions per prompt", group_size)
        return run_batched(
            rows,
            schema,
            extractor.extract_grouped,
            description_col,
            on_error,
            group_size,
            on_progress,
        )
    if batch_size != 1 and hasattr(extractor, "extract_batch"):
        size = batch_size or len(rows)  # 0 → the whole frame in one call
        logger.info("Running extraction in batches of %d", size)
        return run_batched(
            rows,
            schema,
            extractor.extract_batch,
            description_col,
            on_error,
            size,
            on_progress,
        )
    logger.info("Running extraction per row")
    return run_per_row(rows, schema, extractor, description_col, on_error, on_progress)


def run_deduped(
    rows,
    schema,
    extractor,
    description_col,
    on_error,
    batch_size,
    group_size,
    near_threshold,
    embed,
    cache,
    on_progress=None,
) -> tuple[list[dict], int]:
    """Extract once per distinct description and broadcast (Step 0).

    Rows are keyed by their normalised description; only the first row of each
    key (its representative) goes to the model, after the cache is consulted.
    A failed representative therefore nulls every row of its group — failures
    are counted on the expanded rows so the summary stays honest.
    """
    descriptions = [row[description_col] or "" for row in rows]
    keys = [description_key(d) for d in descriptions]
    if near_threshold is not None:
        keys = merge_near_duplicates(descriptions, keys, near_threshold, embed)

    first: dict[str, int] = {}
    for i, key in enumerate(keys):
        first.setdefault(key, i)
    cached: dict[str, dict] = {}
    if cache is not None:
        cached = {key: feats for key in first if (feats := cache.get(key))}
    todo = [key for key in first if key not in cached]
    logger.info(
        "dedup: %d rows → %d distinct (%d cached, %d to extract)",
        len(rows),
        len(first),
        len(cached),
        len(todo),
    )

    fresh, _ = run_rows(
        [rows[first[key]] for key in todo],
        schema,
        extractor,
        description_col,
        on_error,
        batch_size,
        group_size,
        on_progress,
    )
    if cache is not None:
        cache.put_many(dict(zip(todo, fresh)))

    by_key = {**cached, **dict(zip(todo, fresh))}
    records = [by_key[key] for key in keys]
    return records, sum(1 for record in records if not record)


def run_per_row(
    rows, schema, extractor, description_col, on_error, on_progress=None
) -> tuple[list[dict], int]:
    records: list[dict] = []
    failures = 0
    total = len(rows)
    for i, row in enumerate(rows):
        try:
            feats = extractor.extract(schema, row[description_col] or "")
        except Exception as exc:
            if on_error == "raise":
                raise
            logger.warning("row %d extraction failed: %s", i, exc)
            failures += 1
            feats = {}
        records.append(feats)
        if on_progress:
            on_progress(i + 1, total)
    return records, failures


def run_batched(
    rows, schema, extract, description_col, on_error, batch_size, on_progress=None
) -> tuple[list[dict], int]:
    """Drive chunked extraction through ``extract(schema, descriptions)``.

    ``extract`` is the chunk-shaped method to call — ``extract_batch`` (engine
    batching) or ``extract_grouped`` (prompt batching); the orchestration and
    error handling are identical for both.
    """
    records: list[dict] = []
    failures = 0
    total = len(rows)
    for start in range(0, total, batch_size):
        chunk = rows[start : start + batch_size]
        descriptions = [r[description_col] or "" for r in chunk]
        try:
            feats_list = extract(schema, descriptions)
        except Exception as exc:
            if on_error == "raise":
                raise
            logger.warning("batch at row %d failed: %s", start, exc)
            failures += len(chunk)
            feats_list = [{} for _ in chunk]
        records.extend(feats_list)
        if on_progress:
            on_progress(min(start + batch_size, total), total)
    return records, failures


def stamp_provenance(
    df: pl.DataFrame, *, model_id: str, schema_domain: str, at: str
) -> pl.DataFrame:
    """Add provenance columns so an output frame is self-describing.

    Records which model and schema produced the features, and when — pass a
    pinned ``model_id`` (with revision) and an ISO timestamp from the caller so
    the function stays pure/deterministic.
    """
    return df.with_columns(
        pl.lit(model_id).alias("_model"),
        pl.lit(schema_domain).alias("_schema"),
        pl.lit(at).alias("_extracted_at"),
    )


def records_to_frame(records: Iterable[dict], columns: list[str]) -> pl.DataFrame:
    # Build column by column: guarantees the right height and stable columns
    # even when a batch triggered no conditional field (or a row failed).
    rows = list(records)
    data = {col: [rec.get(col) for rec in rows] for col in columns}
    return pl.DataFrame(data)
