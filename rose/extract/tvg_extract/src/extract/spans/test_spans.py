"""A4's pure pieces: entity labels, span→record reduction, frame orchestration.

The GLiNER model is injected (``GLiNERLocator(model=...)``) so no weights or
optional package are needed here — the same seam the CLI loads lazily.
"""

from __future__ import annotations

import polars as pl
import pytest

from extract.domains import get_schema
from extract.schema import ExtractionField, ExtractionSchema, FieldKind
from extract.spans import (
    GLiNERLocator,
    entity_labels,
    locate_frame,
    spans_to_record,
)


def test_entity_labels_cover_fields_and_children_with_spaced_names():
    labels = entity_labels(get_schema("clothing"))
    assert labels["category"] == "category"
    assert labels["primary colour"] == "primary_colour"
    # Conditional children (dress_type etc.) are included.
    assert "dress type" in labels


def weight_schema() -> ExtractionSchema:
    return ExtractionSchema(
        domain="demo",
        description="demo",
        fields=(
            ExtractionField("category", "kind", FieldKind.CATEGORICAL, ("kettle",)),
            ExtractionField("weight", "weight", FieldKind.NUMBER),
        ),
    )


def test_spans_to_record_keeps_best_span_and_coerces_through_the_schema():
    schema = weight_schema()
    labels = entity_labels(schema)
    entities = [
        {"label": "category", "text": "Kettle", "score": 0.9},
        {"label": "category", "text": "teapot", "score": 0.3},  # lower → dropped
        {"label": "weight", "text": "2.5 kg", "score": 0.8},
        {"label": "irrelevant", "text": "x", "score": 0.9},  # unknown → ignored
    ]
    record = spans_to_record(entities, labels, schema)
    assert record["category"] == "kettle"  # categorical snapping
    assert record["weight"] == 2.5  # number parsed out of the span's unit


def test_spans_to_record_abstains_on_unfound_fields():
    schema = weight_schema()
    record = spans_to_record([], entity_labels(schema), schema)
    assert record == {"category": None, "weight": None}


class FakeGLiNER:
    """Tags 'category' on the keyword and 'weight' when a quantity appears."""

    def __init__(self):
        self.batches: list[list[str]] = []

    def batch_predict_entities(self, texts, labels, threshold=0.4):
        self.batches.append(list(texts))
        out = []
        for text in texts:
            entities = []
            if "kettle" in text.lower():
                entities.append({"label": "category", "text": "kettle", "score": 0.9})
            if "kg" in text:
                entities.append({"label": "weight", "text": "2.5 kg", "score": 0.8})
            out.append(entities)
        return out


def test_locate_frame_dedups_broadcasts_and_aligns_columns():
    df = pl.DataFrame(
        {
            "id": [1, 2, 3],
            "description": [
                "Steel kettle, 2.5 kg",
                "STEEL  Kettle, 2.5 kg",  # duplicate key of row 0
                "a mystery item",
            ],
        }
    )
    fake = FakeGLiNER()
    locator = GLiNERLocator(model=fake)
    out = locate_frame(df, locator, weight_schema())
    # Distinct descriptions located once…
    assert len(fake.batches) == 1
    assert len(fake.batches[0]) == 2
    # …and broadcast; the mystery row abstains to nulls, frame stays aligned.
    assert out.get_column("category").to_list() == ["kettle", "kettle", None]
    assert out.get_column("weight").to_list() == [2.5, 2.5, None]


def test_locate_frame_missing_description_column_raises():
    with pytest.raises(ValueError, match="description"):
        locate_frame(
            pl.DataFrame({"id": [1]}),
            GLiNERLocator(model=FakeGLiNER()),
            weight_schema(),
        )
