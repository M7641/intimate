"""Evaluation metrics for contrastive embeddings."""

from __future__ import annotations

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch import Tensor


def knn_accuracy(
    train_embeddings: Tensor,
    train_labels: Tensor,
    test_embeddings: Tensor,
    test_labels: Tensor,
    k: int = 20,
) -> float:
    """k-nearest neighbors classification accuracy.

    Classifies test samples by majority vote of their k nearest
    training neighbors in embedding space (cosine distance).
    """
    train_norm = F.normalize(train_embeddings, dim=-1)
    test_norm = F.normalize(test_embeddings, dim=-1)

    # Cosine similarity → higher = closer
    sim = test_norm @ train_norm.T  # (N_test, N_train)
    _, indices = sim.topk(k, dim=1)  # (N_test, k)

    neighbor_labels = train_labels[indices]  # (N_test, k)
    predictions = neighbor_labels.mode(dim=1).values
    return (predictions == test_labels).float().mean().item()


def linear_probe(
    train_embeddings: Tensor,
    train_labels: Tensor,
    test_embeddings: Tensor,
    test_labels: Tensor,
    epochs: int = 100,
    lr: float = 1e-2,
) -> float:
    """Train a linear classifier on frozen embeddings and return test accuracy."""
    device = train_embeddings.device
    num_classes = int(train_labels.max().item()) + 1
    dim = train_embeddings.shape[1]

    classifier = nn.Linear(dim, num_classes).to(device)
    optimizer = torch.optim.Adam(classifier.parameters(), lr=lr)

    # Detach embeddings — we're probing, not fine-tuning
    x_train = train_embeddings.detach()
    x_test = test_embeddings.detach()

    for _ in range(epochs):
        logits = classifier(x_train)
        loss = F.cross_entropy(logits, train_labels)
        optimizer.zero_grad()
        loss.backward()
        optimizer.step()

    with torch.no_grad():
        preds = classifier(x_test).argmax(dim=1)
    return (preds == test_labels).float().mean().item()


def alignment(z1: Tensor, z2: Tensor, alpha: float = 2.0) -> float:
    """Alignment metric (Wang & Isola, 2020).

    Measures how close positive pairs are on the hypersphere.
    Lower is better. z1[i] and z2[i] should be positive pairs.
    """
    z1 = F.normalize(z1, dim=-1)
    z2 = F.normalize(z2, dim=-1)
    return (z1 - z2).norm(dim=-1).pow(alpha).mean().item()


def uniformity(embeddings: Tensor, t: float = 2.0) -> float:
    """Uniformity metric (Wang & Isola, 2020).

    Measures how uniformly embeddings are distributed on the hypersphere.
    Lower (more negative) is better.
    """
    z = F.normalize(embeddings, dim=-1)
    sq_dists = torch.cdist(z, z, p=2.0).pow(2)
    # Exclude self-pairs
    n = z.shape[0]
    mask = ~torch.eye(n, dtype=torch.bool, device=z.device)
    sq_dists = sq_dists[mask].view(n, n - 1)
    return (
        torch.logsumexp(-t * sq_dists, dim=1).mean().item()
        - torch.tensor(n - 1.0).log().item()
    )


def silhouette(embeddings: Tensor, labels: Tensor) -> float:
    """Silhouette score for clustering quality.

    Measures how well samples cluster by label. Range: [-1, 1], higher is better.
    Uses cosine distance. Pure-torch implementation (no sklearn needed).
    """
    z = F.normalize(embeddings, dim=-1)
    # Cosine distance = 1 - cosine_similarity
    dist = 1.0 - (z @ z.T)

    unique_labels = labels.unique()
    n = z.shape[0]
    scores = torch.zeros(n, device=z.device)

    for i in range(n):
        own_label = labels[i]
        own_mask = labels == own_label
        own_mask[i] = False  # exclude self

        # a(i) = mean distance to same-cluster points
        if own_mask.sum() > 0:
            a_i = dist[i][own_mask].mean()
        else:
            a_i = torch.tensor(0.0, device=z.device)

        # b(i) = min mean distance to any other cluster
        b_i = torch.tensor(float("inf"), device=z.device)
        for label in unique_labels:
            if label == own_label:
                continue
            other_mask = labels == label
            if other_mask.sum() > 0:
                mean_dist = dist[i][other_mask].mean()
                b_i = torch.min(b_i, mean_dist)

        scores[i] = (b_i - a_i) / torch.max(a_i, b_i).clamp(min=1e-8)

    return scores.mean().item()
