"""Tests for batched embedding extraction via encode_all."""

import torch
import torch.nn as nn

from mana.mimic import (
    Compose,
    DeepSetsEncoder,
    FeatureNoise,
    MeanAggregator,
    MLPElementEncoder,
    MLPProjector,
    MimicModel,
    NTXentLoss,
    SetDataset,
    set_dataloader,
)


def _make_model() -> MimicModel:
    return MimicModel(
        encoder=DeepSetsEncoder(
            element_encoder=MLPElementEncoder(4, 16, 16),
            aggregator=MeanAggregator(),
            rho=nn.Sequential(nn.Linear(16, 8), nn.ReLU()),
            output_dim=8,
        ),
        projector=MLPProjector(8, 8, 8),
        augmentation=Compose([FeatureNoise(std=0.1)]),
        loss_fn=NTXentLoss(temperature=0.1),
    )


def _make_data(n: int = 12):
    sets = [torch.randn(torch.randint(2, 6, (1,)).item(), 4) for _ in range(n)]
    ds = SetDataset(sets)
    return ds, set_dataloader(ds, batch_size=4, shuffle=False)


class TestEncodeAll:
    def test_output_shape(self) -> None:
        model = _make_model()
        ds, dl = _make_data(n=10)
        emb = model.encode_all(dl)
        assert emb.shape == (10, 8)

    def test_matches_manual_loop(self) -> None:
        model = _make_model()
        _, dl = _make_data(n=8)

        # Manual
        parts = []
        for batch in dl:
            parts.append(model.encode(batch.x, batch.mask))
        manual = torch.cat(parts, dim=0)

        # encode_all
        auto = model.encode_all(dl)
        assert torch.allclose(manual, auto)

    def test_restores_training_mode(self) -> None:
        model = _make_model()
        model.train()
        _, dl = _make_data(n=4)
        model.encode_all(dl)
        assert model.training

    def test_no_grad(self) -> None:
        model = _make_model()
        _, dl = _make_data(n=4)
        emb = model.encode_all(dl)
        assert not emb.requires_grad
