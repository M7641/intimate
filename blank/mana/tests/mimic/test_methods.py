"""Tests for the axis-B Method abstraction and its implementations."""

import torch
import torch.nn as nn

from mana.mimic import (
    Compose,
    ContrastiveMethod,
    DeepSetsEncoder,
    FeatureNoise,
    MaskedSetAutoencoder,
    MeanAggregator,
    MimicModel,
    MLPElementEncoder,
    MLPProjector,
    NTXentLoss,
    SetDataset,
    SSLTrainer,
    TrainConfig,
    make_decoder,
    set_dataloader,
)


def _synthetic_data(n: int = 32, dim: int = 8) -> list[torch.Tensor]:
    return [torch.randn(torch.randint(3, 11, (1,)).item(), dim) for _ in range(n)]


def _deep_sets(dim: int, out: int) -> DeepSetsEncoder:
    return DeepSetsEncoder(
        element_encoder=MLPElementEncoder(dim, 32, 32),
        aggregator=MeanAggregator(),
        rho=nn.Sequential(nn.Linear(32, out), nn.ReLU()),
        output_dim=out,
    )


class TestBackwardCompatibility:
    def test_mimic_model_is_contrastive_method(self) -> None:
        assert MimicModel is ContrastiveMethod


class TestMaskedSetAutoencoder:
    def test_trains_and_loss_decreases(self) -> None:
        torch.manual_seed(42)
        dim, enc_out = 8, 32
        ds = SetDataset(_synthetic_data(n=32, dim=dim))
        dl = set_dataloader(ds, batch_size=16, shuffle=True)

        method = MaskedSetAutoencoder(
            encoder=_deep_sets(dim, enc_out),
            # Reconstructs mean ⊕ variance → target dim is 2 * D.
            decoder=make_decoder(enc_out, 32, 2 * dim),
            mask_ratio=0.5,
        )

        result = SSLTrainer(method, TrainConfig(epochs=20, lr=1e-3, verbose=False)).fit(
            dl
        )
        assert len(result.epoch_losses) == 20
        assert result.epoch_losses[-1] < result.epoch_losses[0]

    def test_encode_inference_is_method_agnostic(self) -> None:
        """The same inference surface works for a non-contrastive method."""
        torch.manual_seed(0)
        dim, enc_out = 8, 16
        ds = SetDataset(_synthetic_data(n=16, dim=dim))
        dl = set_dataloader(ds, batch_size=8, shuffle=False)

        method = MaskedSetAutoencoder(
            encoder=_deep_sets(dim, enc_out),
            decoder=make_decoder(enc_out, 16, 2 * dim),
        )
        emb = method.encode_all(dl)
        assert emb.shape == (16, enc_out)

    def test_corrupt_never_empties_a_set(self) -> None:
        method = MaskedSetAutoencoder(
            encoder=_deep_sets(4, 8),
            decoder=make_decoder(8, 8, 8),
            mask_ratio=1.0,  # try to drop everything
        )
        x = torch.randn(3, 6, 4)
        mask = torch.ones(3, 6, dtype=torch.bool)
        new_mask = method._corrupt(x, mask)
        assert (new_mask.sum(dim=1) >= 1).all()


class TestAnyEncoderAnyMethod:
    def test_contrastive_method_still_trains(self) -> None:
        """ContrastiveMethod under the renamed trainer behaves as before."""
        torch.manual_seed(1)
        ds = SetDataset(_synthetic_data(n=32, dim=8))
        dl = set_dataloader(ds, batch_size=16, shuffle=True)

        method = ContrastiveMethod(
            encoder=_deep_sets(8, 32),
            projector=MLPProjector(32, 16, 16),
            augmentation=Compose([FeatureNoise(std=0.1)]),
            loss_fn=NTXentLoss(temperature=0.1),
        )
        result = SSLTrainer(method, TrainConfig(epochs=15, lr=1e-3, verbose=False)).fit(
            dl
        )
        assert result.epoch_losses[-1] < result.epoch_losses[0]
