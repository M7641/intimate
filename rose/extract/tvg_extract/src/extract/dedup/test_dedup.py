"""The pure pieces of Step 0: keys, leader clustering, the JSONL cache."""

from __future__ import annotations

import torch

from extract.dedup import (
    ExtractionCache,
    description_key,
    merge_near_duplicates,
    near_duplicate_leaders,
    normalise,
)


def test_normalise_collapses_case_and_whitespace():
    assert normalise("  Red\nMaxi   Dress ") == "red maxi dress"
    assert normalise(None) == ""


def test_description_key_is_stable_across_trivial_variants():
    assert description_key("Red  Maxi Dress") == description_key("red maxi\ndress")
    assert description_key("red dress") != description_key("blue dress")


def test_near_duplicate_leaders_groups_by_cosine():
    vectors = torch.tensor([[1.0, 0.0], [1.0, 0.0], [0.0, 1.0]])
    assert near_duplicate_leaders(vectors, threshold=0.9) == [0, 0, 2]


def test_near_duplicate_leaders_threshold_gates_membership():
    # 45° apart → cosine ≈ 0.707: one group at 0.5, two groups at 0.9.
    vectors = torch.tensor([[1.0, 0.0], [0.7071, 0.7071]])
    assert near_duplicate_leaders(vectors, threshold=0.5) == [0, 0]
    assert near_duplicate_leaders(vectors, threshold=0.9) == [0, 1]


def fake_embed(texts):
    return torch.tensor([[1.0, 0.0] if "dress" in t else [0.0, 1.0] for t in texts])


def test_merge_near_duplicates_remaps_members_to_the_leader_key():
    descriptions = ["a red dress", "a red dress :)", "blue trainers"]
    keys = [description_key(d) for d in descriptions]
    merged = merge_near_duplicates(descriptions, keys, 0.9, fake_embed)
    # The two dress variants share the FIRST one's key; trainers keep theirs.
    assert merged[0] == merged[1] == keys[0]
    assert merged[2] == keys[2]


def test_merge_near_duplicates_skips_embedding_below_two_distinct():
    def boom(texts):
        raise AssertionError("embed must not be called")

    keys = [description_key("a red dress")] * 2
    assert (
        merge_near_duplicates(["a red dress", "A red dress"], keys, 0.9, boom) == keys
    )


def test_cache_roundtrips_across_instances(tmp_path):
    path = tmp_path / "cache.jsonl"
    cache = ExtractionCache(path, model_id="m", schema_domain="clothing")
    assert cache.get("k") is None
    cache.put_many({"k": {"category": "dress"}})

    again = ExtractionCache(path, model_id="m", schema_domain="clothing")
    assert again.get("k") == {"category": "dress"}


def test_cache_is_scoped_to_model_and_schema(tmp_path):
    path = tmp_path / "cache.jsonl"
    ExtractionCache(path, model_id="m1", schema_domain="s").put_many({"k": {"a": 1}})
    assert ExtractionCache(path, model_id="m2", schema_domain="s").get("k") is None
    assert ExtractionCache(path, model_id="m1", schema_domain="other").get("k") is None


def test_cache_never_stores_failures(tmp_path):
    path = tmp_path / "cache.jsonl"
    cache = ExtractionCache(path, model_id="m", schema_domain="s")
    cache.put_many({"k": {}})  # a failed extraction — must stay retryable
    assert cache.get("k") is None
    assert not path.exists()


def test_cache_survives_a_torn_line(tmp_path):
    path = tmp_path / "cache.jsonl"
    good = '{"key": "k", "model": "m", "schema": "s", "features": {"a": 1}}'
    path.write_text(good + "\n{torn wri", encoding="utf-8")
    cache = ExtractionCache(path, model_id="m", schema_domain="s")
    assert cache.get("k") == {"a": 1}
