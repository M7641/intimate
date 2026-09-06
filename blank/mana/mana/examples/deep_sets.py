"""Deep Sets on synthetic clustered data.

Usage:
    uv run mana examples deep-sets
"""

from __future__ import annotations

import torch.nn as nn

from mana import mimic

from ._common import evaluate, header, make_clustered_data, train


def main() -> None:
    header("Deep Sets")

    feature_dim = 10
    dataset, dataloader, labels, total = make_clustered_data(
        clusters=5,
        samples_per_cluster=80,
        feature_dim=feature_dim,
        batch_size=128,
    )

    model = mimic.MimicModel(
        encoder=mimic.DeepSetsEncoder(
            element_encoder=mimic.MLPElementEncoder(feature_dim, 64, 64),
            aggregator=mimic.MeanAggregator(),
            rho=nn.Sequential(nn.Linear(64, 128), nn.ReLU(), nn.Linear(128, 128)),
            output_dim=128,
        ),
        projector=mimic.MLPProjector(128, 64, 64),
        augmentation=mimic.Compose(
            [
                mimic.SubsetSample(keep_fraction=0.8),
                mimic.FeatureNoise(std=0.1),
            ]
        ),
        loss_fn=mimic.NTXentLoss(temperature=0.1),
    )

    train("Deep Sets", model, dataloader, epochs=50, scheduler="cosine")
    evaluate(model, dataset, labels, clusters=5, total=total)


if __name__ == "__main__":
    main()
