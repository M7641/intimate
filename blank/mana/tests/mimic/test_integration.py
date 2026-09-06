"""Integration test: full pipeline on synthetic data."""

import torch
import torch.nn as nn

from mana.mimic import (
    Compose,
    ContrastiveTrainer,
    DeepSetsEncoder,
    FeatureNoise,
    MeanAggregator,
    MimicModel,
    MLPElementEncoder,
    MLPProjector,
    NTXentLoss,
    SetDataset,
    SetTransformerEncoder,
    SubsetSample,
    TrainConfig,
    make_decoder,
    set_dataloader,
)


def _synthetic_data(
    n: int = 64, min_size: int = 3, max_size: int = 10, dim: int = 8
) -> list[torch.Tensor]:
    return [
        torch.randn(torch.randint(min_size, max_size + 1, (1,)).item(), dim)
        for _ in range(n)
    ]


class TestDeepSetsIntegration:
    def test_loss_decreases(self) -> None:
        torch.manual_seed(42)
        data = _synthetic_data(n=32, dim=8)
        ds = SetDataset(data)
        dl = set_dataloader(ds, batch_size=16, shuffle=True)

        model = MimicModel(
            encoder=DeepSetsEncoder(
                element_encoder=MLPElementEncoder(8, 32, 32),
                aggregator=MeanAggregator(),
                rho=nn.Sequential(nn.Linear(32, 32), nn.ReLU()),
                output_dim=32,
            ),
            projector=MLPProjector(32, 16, 16),
            augmentation=Compose([FeatureNoise(std=0.1)]),
            loss_fn=NTXentLoss(temperature=0.1),
        )

        result = ContrastiveTrainer(
            model, TrainConfig(epochs=20, lr=1e-3, verbose=False)
        ).fit(dl)
        assert len(result.epoch_losses) == 20
        # Loss should decrease from start to end
        assert result.epoch_losses[-1] < result.epoch_losses[0]

    def test_encode_after_training(self) -> None:
        torch.manual_seed(0)
        data = _synthetic_data(n=16, dim=8)
        ds = SetDataset(data)
        dl = set_dataloader(ds, batch_size=8, shuffle=False)

        model = MimicModel(
            encoder=DeepSetsEncoder(
                element_encoder=MLPElementEncoder(8, 16, 16),
                aggregator=MeanAggregator(),
                rho=nn.Sequential(nn.Linear(16, 16), nn.ReLU()),
                output_dim=16,
            ),
            projector=MLPProjector(16, 8, 8),
            augmentation=Compose([FeatureNoise(std=0.05)]),
            loss_fn=NTXentLoss(temperature=0.1),
        )

        ContrastiveTrainer(model, TrainConfig(epochs=5, lr=1e-3, verbose=False)).fit(dl)

        # Can encode a batch
        batch = next(iter(dl))
        emb = model.encode(batch.x, batch.mask)
        assert emb.shape == (8, 16)


class TestReconstructionIntegration:
    def test_training_with_reconstruction_loss_decreases(self) -> None:
        torch.manual_seed(42)
        data = _synthetic_data(n=32, dim=8)
        ds = SetDataset(data)
        dl = set_dataloader(ds, batch_size=16, shuffle=True)

        enc_dim = 32
        model = MimicModel(
            encoder=DeepSetsEncoder(
                element_encoder=MLPElementEncoder(8, 32, 32),
                aggregator=MeanAggregator(),
                rho=nn.Sequential(nn.Linear(32, enc_dim), nn.ReLU()),
                output_dim=enc_dim,
            ),
            projector=MLPProjector(enc_dim, 16, 16),
            augmentation=Compose([FeatureNoise(std=0.1)]),
            loss_fn=NTXentLoss(temperature=0.1),
            decoder=make_decoder(enc_dim, 16, 8),
            reconstruction_weight=0.1,
        )

        result = ContrastiveTrainer(
            model, TrainConfig(epochs=20, lr=1e-3, verbose=False)
        ).fit(dl)
        assert len(result.epoch_losses) == 20
        assert result.epoch_losses[-1] < result.epoch_losses[0]


class TestSetTransformerIntegration:
    def test_loss_decreases(self) -> None:
        torch.manual_seed(42)
        data = _synthetic_data(n=32, dim=8)
        ds = SetDataset(data)
        dl = set_dataloader(ds, batch_size=16, shuffle=True)

        model = MimicModel(
            encoder=SetTransformerEncoder(
                input_dim=8, dim=32, output_dim=32, num_heads=2, num_inducing_points=4
            ),
            projector=MLPProjector(32, 16, 16),
            augmentation=Compose(
                [SubsetSample(keep_fraction=0.8), FeatureNoise(std=0.05)]
            ),
            loss_fn=NTXentLoss(temperature=0.1),
        )

        result = ContrastiveTrainer(
            model, TrainConfig(epochs=20, lr=1e-3, verbose=False)
        ).fit(dl)
        assert len(result.epoch_losses) == 20
        assert result.epoch_losses[-1] < result.epoch_losses[0]
