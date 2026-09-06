"""Tests for evaluation metrics."""

import torch
import torch.nn.functional as F

from mana.mimic.evaluate import (
    alignment,
    knn_accuracy,
    linear_probe,
    silhouette,
    uniformity,
)


class TestKNNAccuracy:
    def test_perfect_clusters(self) -> None:
        """Well-separated clusters should give high kNN accuracy."""
        train_emb = torch.cat(
            [
                torch.randn(20, 16) + torch.tensor([5.0] + [0.0] * 15),
                torch.randn(20, 16) + torch.tensor([0.0, 5.0] + [0.0] * 14),
            ]
        )
        train_labels = torch.cat([torch.zeros(20), torch.ones(20)]).long()

        test_emb = torch.cat(
            [
                torch.randn(5, 16) + torch.tensor([5.0] + [0.0] * 15),
                torch.randn(5, 16) + torch.tensor([0.0, 5.0] + [0.0] * 14),
            ]
        )
        test_labels = torch.cat([torch.zeros(5), torch.ones(5)]).long()

        acc = knn_accuracy(train_emb, train_labels, test_emb, test_labels, k=5)
        assert acc > 0.8

    def test_returns_float(self) -> None:
        emb = torch.randn(10, 8)
        labels = torch.randint(0, 2, (10,))
        acc = knn_accuracy(emb, labels, emb, labels, k=3)
        assert isinstance(acc, float)
        assert 0.0 <= acc <= 1.0


class TestLinearProbe:
    def test_separable_data(self) -> None:
        """Linearly separable data should give high probe accuracy."""
        torch.manual_seed(42)
        train_emb = torch.cat([torch.randn(30, 8) + 3, torch.randn(30, 8) - 3])
        train_labels = torch.cat([torch.zeros(30), torch.ones(30)]).long()
        test_emb = torch.cat([torch.randn(10, 8) + 3, torch.randn(10, 8) - 3])
        test_labels = torch.cat([torch.zeros(10), torch.ones(10)]).long()

        acc = linear_probe(train_emb, train_labels, test_emb, test_labels, epochs=50)
        assert acc > 0.8

    def test_returns_float(self) -> None:
        emb = torch.randn(20, 8)
        labels = torch.randint(0, 2, (20,))
        acc = linear_probe(emb, labels, emb, labels, epochs=10)
        assert isinstance(acc, float)


class TestAlignment:
    def test_identical_pairs_zero(self) -> None:
        """Identical pairs should have zero alignment (distance)."""
        z = F.normalize(torch.randn(10, 16), dim=-1)
        a = alignment(z, z)
        assert a < 0.01

    def test_random_pairs_positive(self) -> None:
        """Random pairs should have positive alignment."""
        z1 = F.normalize(torch.randn(10, 16), dim=-1)
        z2 = F.normalize(torch.randn(10, 16), dim=-1)
        a = alignment(z1, z2)
        assert a > 0.1


class TestUniformity:
    def test_returns_float(self) -> None:
        z = F.normalize(torch.randn(20, 8), dim=-1)
        u = uniformity(z)
        assert isinstance(u, float)

    def test_clustered_worse_than_spread(self) -> None:
        """Highly clustered embeddings should have worse uniformity than spread ones."""
        torch.manual_seed(42)
        # Clustered: all near the same point
        clustered = F.normalize(torch.randn(20, 8) * 0.01 + 1.0, dim=-1)
        # Spread: diverse directions
        spread = F.normalize(torch.randn(20, 8), dim=-1)
        u_clustered = uniformity(clustered)
        u_spread = uniformity(spread)
        # Lower uniformity = more uniform (better). Clustered should be higher (worse).
        assert u_clustered > u_spread


class TestSilhouette:
    def test_well_separated_clusters(self) -> None:
        """Well-separated clusters should have high silhouette score."""
        emb = torch.cat(
            [
                torch.randn(20, 8) + torch.tensor([5.0] + [0.0] * 7),
                torch.randn(20, 8) + torch.tensor([0.0, 5.0] + [0.0] * 6),
            ]
        )
        labels = torch.cat([torch.zeros(20), torch.ones(20)]).long()
        s = silhouette(emb, labels)
        assert s > 0.3

    def test_range(self) -> None:
        """Silhouette should be in [-1, 1]."""
        emb = torch.randn(20, 8)
        labels = torch.randint(0, 3, (20,))
        s = silhouette(emb, labels)
        assert -1.0 <= s <= 1.0
