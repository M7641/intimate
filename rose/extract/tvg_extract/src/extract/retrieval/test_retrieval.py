"""A3's pure pieces: label loading, nearest-with-margin, frame orchestration."""

from __future__ import annotations

import json

import polars as pl
import pytest
import torch

from extract.retrieval import (
    classify_frame,
    label_texts,
    load_labels,
    nearest,
)


def test_load_labels_accepts_a_plain_name_list(tmp_path):
    path = tmp_path / "labels.json"
    path.write_text(json.dumps(["dress", "sofa"]))
    assert load_labels(path) == [
        {"name": "dress", "gloss": ""},
        {"name": "sofa", "gloss": ""},
    ]


def test_load_labels_accepts_glossed_objects(tmp_path):
    path = tmp_path / "labels.json"
    path.write_text(json.dumps([{"name": "pump", "gloss": "a slip-on court shoe"}]))
    assert load_labels(path) == [{"name": "pump", "gloss": "a slip-on court shoe"}]


def test_load_labels_accepts_a_discover_taxonomy(tmp_path):
    path = tmp_path / "taxonomy.json"
    path.write_text(json.dumps({"product_type": {"choices": ["dress", "sofa"]}}))
    assert [lab["name"] for lab in load_labels(path)] == ["dress", "sofa"]


def test_load_labels_rejects_an_empty_file(tmp_path):
    path = tmp_path / "labels.json"
    path.write_text("[]")
    with pytest.raises(ValueError, match="no labels"):
        load_labels(path)


def test_label_texts_expand_glosses():
    labels = [{"name": "pump", "gloss": "a slip-on court shoe"}, {"name": "sofa"}]
    assert label_texts(labels) == ["pump — a slip-on court shoe", "sofa"]


def test_nearest_returns_best_similarity_and_margin():
    references = torch.tensor([[1.0, 0.0], [0.0, 1.0]])
    queries = torch.tensor([[0.8, 0.6]])  # closer to ref 0 (cos 0.8 vs 0.6)
    best, sims, margins = nearest(queries, references)
    assert best == [0]
    assert sims[0] == pytest.approx(0.8)
    assert margins[0] == pytest.approx(0.2)  # 0.8 - 0.6: the D1 signal


def test_nearest_margin_is_zero_with_a_single_reference():
    best, sims, margins = nearest(
        torch.tensor([[1.0, 0.0]]), torch.tensor([[1.0, 0.0]])
    )
    assert best == [0]
    assert margins == [0.0]


class FakeEncoder:
    """Keyword-keyed unit vectors; labels and descriptions share the space."""

    model_id = "fake-encoder"

    def __init__(self):
        self.calls: list[list[str]] = []

    def encode_batched(self, texts, batch_size=64, on_progress=None):
        self.calls.append(list(texts))
        return torch.stack(
            [
                torch.tensor([1.0, 0.0]) if "dress" in t else torch.tensor([0.0, 1.0])
                for t in texts
            ]
        )


def test_classify_frame_assigns_labels_and_margins():
    df = pl.DataFrame(
        {
            "description": [
                "a red maxi dress",
                "A Red  Maxi Dress",  # duplicate key of row 0
                "a leather sofa",
            ]
        }
    )
    labels = [{"name": "dress", "gloss": ""}, {"name": "sofa", "gloss": ""}]
    encoder = FakeEncoder()
    out = classify_frame(df, encoder, labels)
    # One encode sweep for labels, one for the two DISTINCT descriptions.
    assert encoder.calls == [["dress", "sofa"], ["a red maxi dress", "a leather sofa"]]
    assert out.get_column("product_type").to_list() == ["dress", "dress", "sofa"]
    # Orthogonal fixtures → full-confidence margins.
    assert out.get_column("margin").to_list() == pytest.approx([1.0, 1.0, 1.0])
    assert out.get_column("similarity").to_list() == pytest.approx([1.0, 1.0, 1.0])


def test_classify_frame_missing_description_column_raises():
    with pytest.raises(ValueError, match="description"):
        classify_frame(pl.DataFrame({"id": [1]}), FakeEncoder(), [{"name": "x"}])
