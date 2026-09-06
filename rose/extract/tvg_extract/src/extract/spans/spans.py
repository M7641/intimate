"""A4 — span extraction: locate, don't generate (README).

Many attribute values appear VERBATIM in the description — colour, material,
brand, weights. There is nothing to generate, only to locate. GLiNER does
this as zero-shot NER: hand it the schema's fields as entity types and it
tags spans in one forward pass per product, no fine-tuning, no decoding loop.

The found spans flow through ``schema.validate``, which already does the
normalisation a span needs: numbers parse out of "2.5 kg", categorical values
lower-case and snap to the choice list. Off-list categorical spans survive as
normalised raw text — composing with retrieval (A3) to embedding-match them
onto the canonical vocabulary is the natural next pattern.

GLiNER is an optional dependency (``uv pip install gliner``), behind a lazy
import, so the core install stays lean.
"""

from __future__ import annotations

from collections.abc import Callable

import polars as pl

from extract.dedup import description_key
from extract.schema import ExtractionSchema

DEFAULT_GLINER = "urchade/gliner_medium-v2.1"
DEFAULT_THRESHOLD = 0.4


def load_gliner():
    """Import GLiNER or raise a clear, actionable error.

    Optional dependency, isolated here so the default install, the test suite
    and the other A-paths never need it — and so tests can monkeypatch the
    seam to drive the locator without model weights.
    """
    try:
        from gliner import GLiNER
    except ImportError as exc:
        msg = (
            "span extraction needs the optional `gliner` package: "
            "`uv pip install gliner`"
        )
        raise RuntimeError(msg) from exc
    return GLiNER


def entity_labels(schema: ExtractionSchema) -> dict[str, str]:
    """Map a GLiNER entity label to each schema field (children included).

    GLiNER works best with short natural-language entity types, so the label
    is the field name with spaces ("primary colour"), not the column name.
    """
    labels: dict[str, str] = {}
    for field in schema.fields:
        labels[field.name.replace("_", " ")] = field.name
        for subs in field.children.values():
            for child in subs:
                labels.setdefault(child.name.replace("_", " "), child.name)
    return labels


def spans_to_record(
    entities: list[dict], labels: dict[str, str], schema: ExtractionSchema
) -> dict:
    """Reduce one text's tagged spans to a validated feature record.

    Keeps the highest-scoring span per field, then lets ``schema.validate``
    do the coercion (numbers out of "2.5 kg", categorical snapping). Fields
    with no span come back null — exactly like an abstaining extractor.
    """
    best: dict[str, tuple[float, str]] = {}
    for entity in entities:
        field = labels.get(entity["label"])
        if field is None:
            continue
        score = float(entity.get("score", 0.0))
        if field not in best or score > best[field][0]:
            best[field] = (score, entity["text"])
    return schema.validate({field: text for field, (_, text) in best.items()})


class GLiNERLocator:
    """Zero-shot span locator with a frame-level surface, mirroring A2/A3.

    ``model`` is injectable for tests; by default the GLiNER checkpoint loads
    lazily on first use.
    """

    def __init__(
        self,
        model_id: str = DEFAULT_GLINER,
        *,
        threshold: float = DEFAULT_THRESHOLD,
        model=None,
    ) -> None:
        self.model_id = model_id
        self.threshold = threshold
        self._model = model

    @property
    def model(self):
        if self._model is None:
            self._model = load_gliner().from_pretrained(self.model_id)
        return self._model

    def locate_batch(
        self, schema: ExtractionSchema, descriptions: list[str]
    ) -> list[dict]:
        """One validated record per description — a forward pass each."""
        labels = entity_labels(schema)
        batches = self.model.batch_predict_entities(
            descriptions, list(labels), threshold=self.threshold
        )
        return [spans_to_record(entities, labels, schema) for entities in batches]


def locate_frame(
    df: pl.DataFrame,
    locator: GLiNERLocator,
    schema: ExtractionSchema,
    *,
    description_col: str = "description",
    batch_size: int = 32,
    on_progress: Callable[[int, int], None] | None = None,
) -> pl.DataFrame:
    """Locate every schema field in a frame; the composable A4 surface.

    Runs the locator once per DISTINCT description (Step-0 dedup — locating
    is deterministic) and broadcasts to duplicate rows. Adds one column per
    schema feature, same shape as the LLM pipeline's output, so the two are
    comparable on the same gold set.
    """
    if description_col not in df.columns:
        msg = f"Expected column missing: {description_col!r}"
        raise ValueError(msg)

    descriptions = [d or "" for d in df.get_column(description_col).to_list()]
    keys = [description_key(d) for d in descriptions]
    first: dict[str, int] = {}
    for i, key in enumerate(keys):
        first.setdefault(key, i)

    distinct = [descriptions[i] for i in first.values()]
    records: list[dict] = []
    for start in range(0, len(distinct), batch_size):
        records.extend(
            locator.locate_batch(schema, distinct[start : start + batch_size])
        )
        if on_progress:
            on_progress(min(start + batch_size, len(distinct)), len(distinct))

    by_key = dict(zip(first, records))
    columns = schema.column_names()
    data = {col: [by_key[key].get(col) for key in keys] for col in columns}
    return pl.concat([df, pl.DataFrame(data)], how="horizontal")
