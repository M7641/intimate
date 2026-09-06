"""The pure pieces of A1: PCA fit/apply and the frame orchestration.

The real encoder needs model weights; ``FakeEncoder`` produces deterministic
vectors per text, which is all ``embed_frame`` cares about.
"""

from __future__ import annotations

import hashlib

import polars as pl
import pytest
import torch

from extract.embeddings import (
    apply_projection,
    default_pooling,
    embed_frame,
    fit_projection,
    pool,
)


def test_default_pooling_matches_the_encoder_family():
    # The Qwen3 embedding family reads the embedding off the last token…
    assert default_pooling("Qwen/Qwen3-Embedding-0.6B") == "last"
    # …the CLS retrieval families off the first…
    assert default_pooling("Alibaba-NLP/gte-modernbert-base") == "cls"
    assert default_pooling("BAAI/bge-base-en-v1.5") == "cls"
    assert default_pooling("Snowflake/snowflake-arctic-embed-m-v1.5") == "cls"
    # …and the older sentence-transformers family wants masked mean pooling.
    assert default_pooling("sentence-transformers/all-MiniLM-L6-v2") == "mean"
    assert default_pooling("intfloat/e5-base-v2") == "mean"


def test_pool_last_takes_the_final_attended_token_right_padding():
    hidden = torch.tensor([[[9.0, 9.0], [3.0, 4.0], [99.0, 99.0]]])
    mask = torch.tensor([[1, 1, 0]])  # last attended token is index 1
    out = pool(hidden, mask, "last")
    assert torch.allclose(out, torch.tensor([[0.6, 0.8]]))


def test_pool_last_takes_the_final_column_left_padding():
    hidden = torch.tensor([[[99.0, 99.0], [9.0, 9.0], [3.0, 4.0]]])
    mask = torch.tensor([[0, 1, 1]])  # left-padded: last column is attended
    out = pool(hidden, mask, "last")
    assert torch.allclose(out, torch.tensor([[0.6, 0.8]]))


def test_pool_cls_takes_the_first_token():
    hidden = torch.tensor([[[3.0, 4.0], [100.0, 100.0]]])  # (1, seq=2, dim=2)
    mask = torch.ones(1, 2, dtype=torch.long)
    out = pool(hidden, mask, "cls")
    assert torch.allclose(out, torch.tensor([[0.6, 0.8]]))  # [3,4] normalised


def test_pool_mean_ignores_padding_tokens():
    hidden = torch.tensor([[[3.0, 4.0], [999.0, 999.0]]])  # second token is pad
    mask = torch.tensor([[1, 0]])
    out = pool(hidden, mask, "mean")
    assert torch.allclose(out, torch.tensor([[0.6, 0.8]]))


def test_pool_rejects_unknown_strategy():
    with pytest.raises(ValueError, match="pooling"):
        pool(torch.zeros(1, 1, 2), torch.ones(1, 1), "max")


def test_fit_projection_shapes_and_json_readiness():
    vectors = torch.randn(20, 8)
    projection = fit_projection(vectors, dim=3)
    assert projection["dim"] == 3
    assert len(projection["mean"]) == 8
    assert len(projection["components"]) == 3
    assert all(len(row) == 8 for row in projection["components"])
    # Lists of floats, not tensors — ready for json.dumps.
    assert isinstance(projection["mean"][0], float)


def test_fit_projection_components_are_orthonormal():
    projection = fit_projection(torch.randn(30, 6), dim=4)
    components = torch.tensor(projection["components"])
    identity = components @ components.T
    assert torch.allclose(identity, torch.eye(4), atol=1e-5)


def test_fit_projection_needs_enough_rows():
    with pytest.raises(ValueError, match="at least 16"):
        fit_projection(torch.randn(5, 8), dim=16)


def test_apply_projection_preserves_planar_structure():
    # Points on a 2-D plane inside 6-D space: a dim=2 PCA must keep their
    # pairwise distances (compression is lossless when the data is truly 2-D).
    basis = torch.linalg.qr(torch.randn(6, 2)).Q  # orthonormal 6x2
    coords = torch.randn(25, 2)
    vectors = coords @ basis.T
    projected = apply_projection(vectors, fit_projection(vectors, dim=2))
    original = torch.cdist(vectors, vectors)
    compressed = torch.cdist(projected, projected)
    assert torch.allclose(original, compressed, atol=1e-4)


class FakeEncoder:
    """Deterministic unit vectors per text — no model weights."""

    model_id = "fake-encoder"

    def __init__(self):
        self.calls: list[list[str]] = []

    def encode_batched(self, texts, batch_size=64, on_progress=None):
        self.calls.append(list(texts))
        rows = []
        for t in texts:
            digest = hashlib.sha256(t.encode()).digest()
            v = torch.tensor([b / 255 for b in digest[:6]])
            rows.append(v / v.norm())
        return torch.stack(rows)


def frame():
    return pl.DataFrame(
        {
            "id": ["1", "2", "3"],
            "description": [
                "a red maxi dress",
                "A Red  Maxi Dress",  # same normalised key as row 0
                "blue running trainers",
            ],
        }
    )


def test_embed_frame_encodes_distinct_descriptions_once():
    encoder = FakeEncoder()
    out, projection = embed_frame(frame(), encoder, dim=2)
    # One encode sweep over the two DISTINCT normalised texts…
    assert encoder.calls == [["a red maxi dress", "blue running trainers"]]
    # …broadcast back to all three rows, at the compressed width.
    vectors = out.get_column("embedding").to_list()
    assert len(vectors) == 3
    assert all(len(v) == 2 for v in vectors)
    assert vectors[0] == vectors[1]  # the duplicate inherits row 0's vector
    assert vectors[0] != vectors[2]
    assert projection["dim"] == 2


def test_embed_frame_reuses_a_stored_projection():
    fitted = embed_frame(frame(), FakeEncoder(), dim=2)[1]
    # A later "run" with the stored projection must land in the same space.
    out, used = embed_frame(frame(), FakeEncoder(), dim=2, projection=fitted)
    assert used is fitted
    first = embed_frame(frame(), FakeEncoder(), dim=2)[0]
    assert (
        out.get_column("embedding").to_list() == first.get_column("embedding").to_list()
    )


def test_embed_frame_missing_description_column_raises():
    with pytest.raises(ValueError, match="description"):
        embed_frame(pl.DataFrame({"id": [1]}), FakeEncoder(), dim=2)
