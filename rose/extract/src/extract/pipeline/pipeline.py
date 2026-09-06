"""Batch orchestration: polars in, enriched polars out."""

from __future__ import annotations

import logging
from collections.abc import Iterable
from typing import Literal

import polars as pl

from extract.extractor import VisionLanguageExtractor
from extract.schema import ExtractionSchema

logger = logging.getLogger(__name__)

OnError = Literal["null", "raise"]


def extract_features(
    df: pl.DataFrame,
    schema: ExtractionSchema,
    extractor: VisionLanguageExtractor,
    *,
    description_col: str = "description",
    image_col: str = "image_path",
    image_url_col: str = "image_url",
    on_error: OnError = "null",
    batch_size: int = 1,
) -> pl.DataFrame:
    """Enrich ``df`` with one column per schema feature.

    The description is required; the image is optional. The image source for a
    row is ``image_col`` (a local path) if set, otherwise ``image_url_col`` (a
    remote URL); if neither is present the row is extracted from text alone.

    ``on_error``: "null" fills a failed row with null values (and logs a warning
    with the reason), "raise" propagates the first failure.

    ``batch_size`` > 1 runs the model on whole batches via
    ``extractor.extract_batch`` — the main throughput lever. It trades per-row
    error isolation for speed: a failed batch nulls its rows (logged).
    """
    if on_error not in ("null", "raise"):
        msg = f"on_error must be 'null' or 'raise', got {on_error!r}"
        raise ValueError(msg)
    if description_col not in df.columns:
        msg = f"Expected column missing: {description_col!r}"
        raise ValueError(msg)
    has_path = image_col in df.columns
    has_url = image_url_col in df.columns

    def image_of(row: dict) -> object | None:
        path = row.get(image_col) if has_path else None
        url = row.get(image_url_col) if has_url else None
        return path or url or None

    rows = list(df.iter_rows(named=True))
    if batch_size > 1 and hasattr(extractor, "extract_batch"):
        records, failures = _run_batched(
            rows, schema, extractor, description_col, image_of, on_error, batch_size
        )
    else:
        records, failures = _run_per_row(
            rows, schema, extractor, description_col, image_of, on_error
        )

    if failures:
        logger.warning("%d/%d rows failed extraction", failures, len(records))

    feats_df = _records_to_frame(records, schema.column_names())
    return pl.concat([df, feats_df], how="horizontal")


def _run_per_row(
    rows, schema, extractor, description_col, image_of, on_error
) -> tuple[list[dict], int]:
    records: list[dict] = []
    failures = 0
    for i, row in enumerate(rows):
        try:
            feats = extractor.extract(schema, row[description_col] or "", image_of(row))
        except Exception as exc:
            if on_error == "raise":
                raise
            logger.warning("row %d extraction failed: %s", i, exc)
            failures += 1
            feats = {}
        records.append(feats)
    return records, failures


def _run_batched(
    rows, schema, extractor, description_col, image_of, on_error, batch_size
) -> tuple[list[dict], int]:
    records: list[dict] = []
    failures = 0
    for start in range(0, len(rows), batch_size):
        chunk = rows[start : start + batch_size]
        descriptions = [r[description_col] or "" for r in chunk]
        images = [image_of(r) for r in chunk]
        try:
            feats_list = extractor.extract_batch(schema, descriptions, images)
        except Exception as exc:
            if on_error == "raise":
                raise
            logger.warning("batch at row %d failed: %s", start, exc)
            failures += len(chunk)
            feats_list = [{} for _ in chunk]
        records.extend(feats_list)
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


def _records_to_frame(records: Iterable[dict], columns: list[str]) -> pl.DataFrame:
    # Build column by column: guarantees the right height and stable columns
    # even when a batch triggered no conditional field (or a row failed).
    rows = list(records)
    data = {col: [rec.get(col) for rec in rows] for col in columns}
    return pl.DataFrame(data)
