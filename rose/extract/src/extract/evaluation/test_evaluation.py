"""Scoring is pure: predictions frame vs gold frame, no model."""

from __future__ import annotations

import polars as pl
import pytest

from extract.evaluation import score_frame
from extract.schema import ExtractionField as F
from extract.schema import ExtractionSchema, FieldKind

SCHEMA = ExtractionSchema(
    "demo",
    "demo",
    fields=(
        F("category", "c", FieldKind.CATEGORICAL, ("dress", "top")),
        F("count", "n", FieldKind.INTEGER),
        F("brand", "b"),
    ),
)


def _report(pred: dict, gold: dict):
    return score_frame(pl.DataFrame(pred), pl.DataFrame(gold), SCHEMA)


def test_perfect_predictions_score_100_percent():
    report = _report(
        {
            "id": [1, 2],
            "category": ["dress", "top"],
            "count": [2, 4],
            "brand": ["x", "y"],
        },
        {
            "id": [1, 2],
            "category": ["dress", "top"],
            "count": [2, 4],
            "brand": ["x", "y"],
        },
    )
    assert report.micro_accuracy == 1.0
    assert report.macro_accuracy == 1.0


def test_categorical_match_is_case_and_spacing_insensitive():
    report = _report(
        {"id": [1], "category": ["dress"]},
        {"id": [1], "category": ["Dress"]},
    )
    [score] = report.per_field
    assert score.field == "category"
    assert score.correct == 1


def test_null_gold_cells_are_excluded_from_support():
    # Only row 1 labels `brand`; row 2's blank must not count against us.
    report = _report(
        {"id": [1, 2], "brand": ["acme", "whatever"]},
        {"id": [1, 2], "brand": ["acme", None]},
    )
    [score] = report.per_field
    assert score.support == 1
    assert score.correct == 1


def test_null_prediction_against_labelled_gold_is_a_miss():
    report = _report(
        {"id": [1], "brand": [None]},
        {"id": [1], "brand": ["acme"]},
    )
    [score] = report.per_field
    assert score.support == 1
    assert score.correct == 0
    assert score.accuracy == 0.0


def test_numeric_field_uses_tolerant_equality():
    report = _report(
        {"id": [1, 2], "count": [8, 7]},
        {"id": [1, 2], "count": [8, 16]},
    )
    [score] = report.per_field
    assert score.support == 2
    assert score.correct == 1  # 8 matches, 7≠16


def test_micro_weights_by_support_macro_does_not():
    # category: 2/2 correct; brand: 0/1 correct.
    report = _report(
        {"id": [1, 2], "category": ["dress", "top"], "brand": ["wrong", None]},
        {"id": [1, 2], "category": ["dress", "top"], "brand": ["acme", None]},
    )
    # micro = 2 correct / 3 supported; macro = mean(1.0, 0.0)
    assert report.micro_accuracy == pytest.approx(2 / 3)
    assert report.macro_accuracy == pytest.approx(0.5)


def test_unlabelled_fields_are_skipped_entirely():
    # gold has no `count` column → no FieldScore for it.
    report = _report(
        {"id": [1], "category": ["dress"], "count": [3]},
        {"id": [1], "category": ["dress"]},
    )
    assert [s.field for s in report.per_field] == ["category"]
