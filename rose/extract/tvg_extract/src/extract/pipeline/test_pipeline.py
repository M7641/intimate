"""Polars orchestration, driven by an injected fake extractor — no model.

``FakeExtractor`` is the whole point: the pipeline takes its extractor by
injection, so we can exercise column alignment, conditional fields and error
handling without loading a multi-GB model.
"""

from __future__ import annotations

import polars as pl
import pytest
import torch

from extract.dedup import ExtractionCache
from extract.domains import get_schema
from extract.pipeline import extract_features, stamp_provenance
from extract.schema import ExtractionSchema


class FakeExtractor:
    """Returns a pre-validated dict per row, keyed by description.

    An unknown description raises — that drives the ``on_error`` paths.
    """

    def __init__(self, schema: ExtractionSchema, answers: dict[str, dict]):
        self.answers = answers
        self.calls: list[str] = []
        self.batches: list[list[str]] = []
        self.groups: list[list[str]] = []

    def extract(self, schema: ExtractionSchema, description: str) -> dict:
        self.calls.append(description)
        if description not in self.answers:
            raise RuntimeError(f"no canned answer for {description!r}")
        # Route through the real validator so the fake matches the real shape.
        return schema.validate(self.answers[description])

    def extract_batch(self, schema, descriptions) -> list[dict]:
        self.batches.append(list(descriptions))
        return [self.extract(schema, d) for d in descriptions]

    def extract_grouped(self, schema, descriptions) -> list[dict]:
        self.groups.append(list(descriptions))
        return [self.extract(schema, d) for d in descriptions]


@pytest.fixture
def clothing_schema():
    return get_schema("clothing")


@pytest.fixture
def sample_df():
    return pl.DataFrame(
        {
            "id": [1, 2],
            "description": ["a red maxi dress", "blue running trainers"],
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
    assert {"id", "description"} <= set(out.columns)
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


def test_missing_description_column_raises(clothing_schema, clothing_extractor):
    df = pl.DataFrame({"id": [1]})
    with pytest.raises(ValueError, match="description"):
        extract_features(df, clothing_schema, clothing_extractor)


def test_unknown_on_error_value_raises(sample_df, clothing_schema, clothing_extractor):
    with pytest.raises(ValueError, match="on_error"):
        extract_features(
            sample_df, clothing_schema, clothing_extractor, on_error="oops"
        )


def test_failed_rows_are_logged(clothing_schema, clothing_extractor, caplog):
    df = pl.DataFrame({"description": ["a red maxi dress", "unanswerable"]})
    with caplog.at_level("WARNING"):
        extract_features(df, clothing_schema, clothing_extractor, on_error="null")
    assert "row 1 extraction failed" in caplog.text
    assert "1/2 rows failed" in caplog.text


def test_on_error_null_fills_failed_rows(clothing_schema, clothing_extractor):
    df = pl.DataFrame(
        {"description": ["a red maxi dress", "something the fake cannot answer"]}
    )
    rows = extract_features(
        df, clothing_schema, clothing_extractor, on_error="null"
    ).to_dicts()
    assert rows[0]["category"] == "dress"
    assert rows[1]["category"] is None  # failed row → all-null, batch survives


def test_on_error_raise_propagates(clothing_schema, clothing_extractor):
    df = pl.DataFrame({"description": ["unanswerable"]})
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


def test_batch_size_zero_sends_the_whole_frame_in_one_call(
    sample_df, clothing_schema, clothing_extractor
):
    out = extract_features(sample_df, clothing_schema, clothing_extractor, batch_size=0)
    assert clothing_extractor.batches == [["a red maxi dress", "blue running trainers"]]
    assert out.height == 2


def test_group_size_routes_through_extract_grouped(
    sample_df, clothing_schema, clothing_extractor
):
    out = extract_features(sample_df, clothing_schema, clothing_extractor, group_size=2)
    # Both rows rode one grouped prompt; engine batching was not used.
    assert clothing_extractor.groups == [["a red maxi dress", "blue running trainers"]]
    assert clothing_extractor.batches == []
    assert out.sort("id").to_dicts()[0]["category"] == "dress"


def test_group_size_and_batch_size_are_mutually_exclusive(
    sample_df, clothing_schema, clothing_extractor
):
    with pytest.raises(ValueError, match="mutually exclusive"):
        extract_features(
            sample_df, clothing_schema, clothing_extractor, batch_size=8, group_size=2
        )


def test_dedup_extracts_each_distinct_description_once(
    clothing_schema, clothing_extractor
):
    df = pl.DataFrame(
        {
            "description": [
                "a red maxi dress",
                "A Red  Maxi Dress",  # same normalised key as row 0
                "blue running trainers",
            ]
        }
    )
    out = extract_features(df, clothing_schema, clothing_extractor)
    # Only the two distinct descriptions hit the model…
    assert clothing_extractor.calls == ["a red maxi dress", "blue running trainers"]
    # …and the duplicate row still gets the broadcast features.
    rows = out.to_dicts()
    assert rows[1]["category"] == "dress"
    assert out.height == 3


def test_no_dedup_extracts_every_row(clothing_schema, clothing_extractor):
    df = pl.DataFrame({"description": ["a red maxi dress", "a red maxi dress"]})
    extract_features(df, clothing_schema, clothing_extractor, dedup=False)
    assert clothing_extractor.calls == ["a red maxi dress", "a red maxi dress"]


def test_dedup_counts_failures_on_the_expanded_rows(
    clothing_schema, clothing_extractor, caplog
):
    # One failed representative nulls BOTH of its duplicate rows — the summary
    # must report 2 failed rows, not 1 failed extraction.
    df = pl.DataFrame({"description": ["unanswerable", "UNANSWERABLE"]})
    with caplog.at_level("WARNING"):
        out = extract_features(df, clothing_schema, clothing_extractor)
    assert "2/2 rows failed" in caplog.text
    assert all(r["category"] is None for r in out.to_dicts())


def test_cache_skips_already_extracted_descriptions(
    tmp_path, clothing_schema, clothing_extractor
):
    path = tmp_path / "cache.jsonl"
    df = pl.DataFrame({"description": ["a red maxi dress"]})
    cache = ExtractionCache(path, model_id="fake", schema_domain="clothing")
    extract_features(df, clothing_schema, clothing_extractor, cache=cache)
    assert clothing_extractor.calls == ["a red maxi dress"]

    # A fresh run with a model-free extractor: everything comes from the cache.
    silent = FakeExtractor(clothing_schema, {})  # raises if ever called
    out = extract_features(
        df,
        clothing_schema,
        silent,
        cache=ExtractionCache(path, model_id="fake", schema_domain="clothing"),
    )
    assert silent.calls == []
    assert out.to_dicts()[0]["category"] == "dress"


def test_near_threshold_merges_near_duplicates(clothing_schema, clothing_extractor):
    df = pl.DataFrame(
        {
            "description": [
                "a red maxi dress",
                "a red maxi dress :)",  # different key, same embedding
                "blue running trainers",
            ]
        }
    )

    def fake_embed(texts):
        return torch.tensor([[1.0, 0.0] if "dress" in t else [0.0, 1.0] for t in texts])

    out = extract_features(
        df, clothing_schema, clothing_extractor, near_threshold=0.9, embed=fake_embed
    )
    # The near-duplicate inherits its leader's features without a model call.
    assert clothing_extractor.calls == ["a red maxi dress", "blue running trainers"]
    assert out.to_dicts()[1]["category"] == "dress"


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
