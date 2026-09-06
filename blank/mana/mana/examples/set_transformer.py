"""Set Transformer on synthetic clustered data.

Usage:
    uv run mana examples set-transformer
"""

from __future__ import annotations

from mana import mimic

from ._common import evaluate, header, make_clustered_data, train


def main() -> None:
    header("Set Transformer")

    feature_dim = 10
    dataset, dataloader, labels, total = make_clustered_data(
        clusters=5,
        samples_per_cluster=80,
        feature_dim=feature_dim,
        batch_size=128,
    )

    model = mimic.MimicModel(
        encoder=mimic.SetTransformerEncoder(
            input_dim=feature_dim,
            dim=64,
            output_dim=128,
            num_heads=4,
            num_inducing_points=8,
        ),
        projector=mimic.MLPProjector(128, 64, 64),
        augmentation=mimic.Compose(
            [
                mimic.FeatureMask(p=0.15),
                mimic.FeatureNoise(std=0.05),
            ]
        ),
        loss_fn=mimic.NTXentLoss(temperature=0.1),
    )

    train("Set Transformer", model, dataloader, epochs=50, scheduler="cosine")
    evaluate(model, dataset, labels, clusters=5, total=total)


if __name__ == "__main__":
    main()
