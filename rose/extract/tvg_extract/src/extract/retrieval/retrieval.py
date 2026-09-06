"""A3 — zero-shot classification by retrieval (README).

Embed every LABEL once, embed each description once, assign by max cosine
similarity: classification becomes an argmax over label embeddings — no
generation, no training, and a new label costs one embedding.

Two details carry the quality and the composability:

- Labels embed better as a short gloss than as a bare noun ("pump — a slip-on
  court shoe" vs "pump"), so a label is ``{name, gloss}`` and the embedded
  text is "name — gloss".
- The returned ``margin`` (best minus second-best similarity) is the
  confidence signal a routing cascade (D1) thresholds on: a small margin
  means the cheap path is unsure and the row belongs on the slow path.
"""

from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path

import polars as pl
import torch

from extract.dedup import description_key, normalise


def load_labels(path: str | Path) -> list[dict]:
    """Load candidate labels from JSON, normalised to ``[{name, gloss}]``.

    Accepts three shapes: a plain list of names, a list of ``{name, gloss}``
    dicts, or the `extract discover` taxonomy output (its ``choices`` list).
    """
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    if isinstance(data, dict):  # discover taxonomy: {"product_type": {...}}
        level = next(iter(data.values()))
        data = level.get("choices", [])
    labels = []
    for item in data:
        if isinstance(item, str):
            labels.append({"name": item, "gloss": ""})
        else:
            labels.append({"name": item["name"], "gloss": item.get("gloss", "")})
    if not labels:
        msg = f"no labels found in {path}"
        raise ValueError(msg)
    return labels


def label_texts(labels: list[dict]) -> list[str]:
    """The text actually embedded per label: the gloss-expanded form."""
    return [
        f"{lab['name']} — {lab['gloss']}" if lab.get("gloss") else lab["name"]
        for lab in labels
    ]


def nearest(
    queries: torch.Tensor, references: torch.Tensor
) -> tuple[list[int], list[float], list[float]]:
    """Argmax cosine of each query over the references, with the margin.

    Unit rows in, so the dot product IS the cosine. Returns per query: the
    best reference index, its similarity, and the margin to the runner-up
    (0.0 when there is only one reference) — the D1 confidence signal.
    """
    sims = queries @ references.T
    top = sims.topk(min(2, references.shape[0]), dim=1)
    best = top.indices[:, 0].tolist()
    best_sim = top.values[:, 0]
    # A single reference has no runner-up: margin 0 (no evidence of
    # separation), never best-minus-nothing.
    runner_up = top.values[:, 1] if references.shape[0] > 1 else best_sim
    margin = best_sim - runner_up
    return best, best_sim.tolist(), margin.tolist()


def classify_frame(
    df: pl.DataFrame,
    encoder,
    labels: list[dict],
    *,
    description_col: str = "description",
    output_col: str = "product_type",
    batch_size: int = 64,
    on_progress: Callable[[int, int], None] | None = None,
) -> pl.DataFrame:
    """Classify a frame's descriptions by retrieval; the composable A3 surface.

    Encodes the labels once and each DISTINCT normalised description once
    (Step-0 dedup), assigns by max cosine, and broadcasts to duplicate rows.
    Adds three columns: ``output_col`` (the label name), ``similarity`` and
    ``margin`` — keep the margins, a routing cascade thresholds on them.
    """
    if description_col not in df.columns:
        msg = f"Expected column missing: {description_col!r}"
        raise ValueError(msg)

    label_vectors = encoder.encode_batched(label_texts(labels), batch_size=batch_size)

    descriptions = [d or "" for d in df.get_column(description_col).to_list()]
    keys = [description_key(d) for d in descriptions]
    first: dict[str, int] = {}
    for i, key in enumerate(keys):
        first.setdefault(key, i)
    distinct = [normalise(descriptions[i]) for i in first.values()]
    vectors = encoder.encode_batched(
        distinct, batch_size=batch_size, on_progress=on_progress
    )

    best, sims, margins = nearest(vectors, label_vectors)
    by_key = {
        key: (labels[best[i]]["name"], sims[i], margins[i])
        for i, key in enumerate(first)
    }
    assigned = [by_key[key] for key in keys]
    return df.with_columns(
        pl.Series(output_col, [a[0] for a in assigned], dtype=pl.Utf8),
        pl.Series("similarity", [a[1] for a in assigned]),
        pl.Series("margin", [a[2] for a in assigned]),
    )
