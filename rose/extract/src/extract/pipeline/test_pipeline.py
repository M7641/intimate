"""Polars orchestration, driven by an injected fake extractor — no model.

``FakeExtractor`` is the whole point: the pipeline takes its extractor by
injection, so we can exercise column alignment, conditional fields and error
handling without loading a multi-GB vision-language model.
"""

from __future__ import annotations

import polars as pl
import pytest

from extract.domains import get_schema
from extract.pipeline import extract_features, stamp_provenance
from extract.schema import ExtractionSchema


class FakeExtractor:
    """Returns a pre-validated dict per row, keyed by description.

    An unknown description raises — that drives the ``on_error`` paths.
    """

    def __init__(self, schema: ExtractionSchema, answers: dict[str, dict]):
        self._answers = answers
        self.calls: list[tuple[str, object]] = []
        self.batches: list[list[str]] = []

    def extract(self, schema: ExtractionSchema, description: str, image) -> dict:
        self.calls.append((description, image))
        if description not in self._answers:
            raise RuntimeError(f"no canned answer for {description!r}")
        # Route through the real validator so the fake matches the real shape.
        return schema.validate(self._answers[description])

    def extract_batch(self, schema, descriptions, images) -> list[dict]:
        self.batches.append(list(descriptions))
        return [self.extract(schema, d, i) for d, i in zip(descriptions, images)]


@pytest.fixture
def clothing_schema():
    return get_schema("clothing")


@pytest.fixture
def sample_df():
    return pl.DataFrame(
        {
            "id": [1, 2],
            "description": ["a red maxi dress", "blue running trainers"],
            "image_path": ["images/1.jpg", "images/2.jpg"],
        }
    )


@pytest.fixture
def clothing_extractor(clothing_schema):
    return FakeExtractor(
        clothing_schema,
        {
            "a red maxi dress": {
                "category": "dress",
                "dress_type": "maxi",
                "primary_colour": "red",
            },
            "blue running trainers": {
                "category": "footwear",
                "footwear_type": "trainers",
                "primary_colour": "blue",
            },
        },
    )


def test_enriches_frame_with_feature_columns(
    sample_df, clothing_schema, clothing_extractor
):
    out = extract_features(sample_df, clothing_schema, clothing_extractor)
    assert {"id", "description", "image_path"} <= set(out.columns)
    assert set(clothing_schema.column_names()) <= set(out.columns)
    assert out.height == sample_df.height


def test_conditional_columns_are_ragged_but_aligned(
    sample_df, clothing_schema, clothing_extractor
):
    # A dress has dress_type but no footwear_type, and vice versa; the frame
    # must still be rectangular, with the absent feature null per row.
    rows = (
        extract_features(sample_df, clothing_schema, clothing_extractor)
        .sort("id")
        .to_dicts()
    )
    assert rows[0]["dress_type"] == "maxi"
    assert rows[0]["footwear_type"] is None
    assert rows[1]["footwear_type"] == "trainers"
    assert rows[1]["dress_type"] is None


def test_prefers_local_image_path_over_url(clothing_schema, clothing_extractor):
    df = pl.DataFrame(
        {
            "description": ["a red maxi dress"],
            "image_path": ["images/local.jpg"],
            "image_url": ["http://example.com/remote.jpg"],
        }
    )
    extract_features(df, clothing_schema, clothing_extractor)
    assert clothing_extractor.calls[0][1] == "images/local.jpg"


def test_falls_back_to_url_when_no_local_path(clothing_schema, clothing_extractor):
    df = pl.DataFrame(
        {
            "description": ["a red maxi dress"],
            "image_path": [""],  # empty → falsy → fall back to the URL
            "image_url": ["http://example.com/remote.jpg"],
        }
    )
    extract_features(df, clothing_schema, clothing_extractor)
    assert clothing_extractor.calls[0][1] == "http://example.com/remote.jpg"


def test_missing_description_column_raises(clothing_schema, clothing_extractor):
    df = pl.DataFrame({"image_path": ["x.jpg"]})
    with pytest.raises(ValueError, match="description"):
        extract_features(df, clothing_schema, clothing_extractor)


def test_text_only_extracts_when_no_image_columns(clothing_schema, clothing_extractor):
    # Image is optional: with no image column, the row is extracted from text
    # alone and the extractor is handed image=None.
    df = pl.DataFrame({"description": ["a red maxi dress"]})
    out = extract_features(df, clothing_schema, clothing_extractor)
    assert out.to_dicts()[0]["category"] == "dress"
    assert clothing_extractor.calls[0][1] is None


def test_unknown_on_error_value_raises(sample_df, clothing_schema, clothing_extractor):
    with pytest.raises(ValueError, match="on_error"):
        extract_features(
            sample_df, clothing_schema, clothing_extractor, on_error="oops"
        )


def test_failed_rows_are_logged(clothing_schema, clothing_extractor, caplog):
    df = pl.DataFrame(
        {
            "description": ["a red maxi dress", "unanswerable"],
            "image_path": ["a.jpg", "b.jpg"],
        }
    )
    with caplog.at_level("WARNING"):
        extract_features(df, clothing_schema, clothing_extractor, on_error="null")
    assert "row 1 extraction failed" in caplog.text
    assert "1/2 rows failed" in caplog.text


def test_on_error_null_fills_failed_rows(clothing_schema, clothing_extractor):
    df = pl.DataFrame(
        {
            "description": ["a red maxi dress", "something the fake cannot answer"],
            "image_path": ["a.jpg", "b.jpg"],
        }
    )
    rows = extract_features(
        df, clothing_schema, clothing_extractor, on_error="null"
    ).to_dicts()
    assert rows[0]["category"] == "dress"
    assert rows[1]["category"] is None  # failed row → all-null, batch survives


def test_on_error_raise_propagates(clothing_schema, clothing_extractor):
    df = pl.DataFrame({"description": ["unanswerable"], "image_path": ["a.jpg"]})
    with pytest.raises(RuntimeError):
        extract_features(df, clothing_schema, clothing_extractor, on_error="raise")


def test_batch_size_routes_through_extract_batch(
    sample_df, clothing_schema, clothing_extractor
):
    out = extract_features(sample_df, clothing_schema, clothing_extractor, batch_size=8)
    # Both rows went through one extract_batch call, results still aligned.
    assert clothing_extractor.batches == [["a red maxi dress", "blue running trainers"]]
    assert out.sort("id").to_dicts()[0]["category"] == "dress"
    assert out.height == 2


def test_stamp_provenance_adds_self_describing_columns():
    df = pl.DataFrame({"id": [1], "category": ["dress"]})
    out = stamp_provenance(
        df,
        model_id="Qwen/x@abc123",
        schema_domain="clothing",
        at="2026-05-29T00:00:00Z",
    )
    row = out.to_dicts()[0]
    assert row["_model"] == "Qwen/x@abc123"
    assert row["_schema"] == "clothing"
    assert row["_extracted_at"] == "2026-05-29T00:00:00Z"
