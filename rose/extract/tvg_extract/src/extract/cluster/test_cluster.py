"""A2's pure pieces: greedy clustering, medoids, terms, frame orchestration."""

from __future__ import annotations

import polars as pl
import torch

from extract.cluster import (
    assign_to_centroids,
    cluster_frame,
    distinctive_terms,
    exemplars,
    greedy_clusters,
    name_clusters,
)

# Two tight groups on orthogonal axes — unambiguous clustering fixtures.
DRESS = torch.tensor([1.0, 0.0])
SOFA = torch.tensor([0.0, 1.0])


def unit(v: torch.Tensor) -> torch.Tensor:
    return v / v.norm()


def test_greedy_clusters_groups_similar_vectors():
    vectors = torch.stack(
        [DRESS, unit(torch.tensor([0.99, 0.1])), SOFA, unit(torch.tensor([0.1, 0.99]))]
    )
    assignment, centroids = greedy_clusters(vectors, threshold=0.8)
    assert assignment == [0, 0, 1, 1]
    assert centroids.shape == (2, 2)


def test_greedy_clusters_threshold_splits():
    vectors = torch.stack([DRESS, SOFA])
    assignment, centroids = greedy_clusters(vectors, threshold=0.5)
    assert assignment == [0, 1]  # orthogonal → never merged
    assert centroids.shape == (2, 2)


def test_exemplars_pick_the_member_nearest_the_centroid():
    near = unit(torch.tensor([0.9, 0.1]))
    vectors = torch.stack([DRESS, near, SOFA])
    assignment, centroids = greedy_clusters(vectors, threshold=0.8)
    medoids = exemplars(vectors, assignment, centroids)
    # Cluster 0's centroid sits between DRESS and `near`; both are members and
    # one of them must be the medoid. Cluster 1 has a single member.
    assert medoids[0] in (0, 1)
    assert medoids[1] == 2


def test_distinctive_terms_rank_cluster_specific_words_first():
    texts = [
        "red cotton maxi dress product",
        "blue cotton midi dress product",
        "large leather corner sofa product",
        "small leather lounge sofa product",
    ]
    terms = distinctive_terms(texts, [0, 0, 1, 1], k=2)
    # "product" appears in both clusters → damped below the specific nouns.
    assert "dress" in terms[0] and "product" not in terms[0]
    assert "sofa" in terms[1] and "product" not in terms[1]


def test_assign_to_centroids_routes_new_vectors():
    centroids = torch.stack([DRESS, SOFA])
    ids, sims = assign_to_centroids(
        torch.stack([unit(torch.tensor([0.9, 0.2]))]), centroids
    )
    assert ids == [0]
    assert sims[0] > 0.9


class FakeEncoder:
    """Keyword-keyed unit vectors — clustering becomes fully predictable."""

    model_id = "fake-encoder"

    def __init__(self):
        self.calls: list[list[str]] = []

    def encode_batched(self, texts, batch_size=64, on_progress=None):
        self.calls.append(list(texts))
        return torch.stack([DRESS if "dress" in t else SOFA for t in texts])


def frame():
    return pl.DataFrame(
        {
            "id": [1, 2, 3],
            "description": [
                "a red maxi dress",
                "A Red  Maxi Dress",  # same normalised key as row 0
                "a leather sofa",
            ],
        }
    )


def test_cluster_frame_dedups_then_broadcasts_cluster_ids():
    encoder = FakeEncoder()
    out, clusters = cluster_frame(frame(), encoder, threshold=0.8)
    # Distinct descriptions encoded once…
    assert encoder.calls == [["a red maxi dress", "a leather sofa"]]
    # …ids broadcast to the duplicate row; sizes count expanded rows.
    assert out.get_column("cluster_id").to_list() == [0, 0, 1]
    rows = clusters.sort("cluster_id").to_dicts()
    assert rows[0]["size"] == 2
    assert rows[0]["exemplar"] == "a red maxi dress"
    assert "dress" in rows[0]["terms"]


def test_name_clusters_applies_the_injected_labeller_once_per_cluster():
    _, clusters = cluster_frame(frame(), FakeEncoder(), threshold=0.8)
    seen: list[str] = []

    def label(exemplar: str) -> str:
        seen.append(exemplar)
        return "dress" if "dress" in exemplar else "sofa"

    named = name_clusters(clusters, label)
    assert named.get_column("name").to_list() == ["dress", "sofa"]
    assert len(seen) == clusters.height  # once per cluster, not per product
