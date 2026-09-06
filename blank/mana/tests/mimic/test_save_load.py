"""Tests for model save/load."""

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
)


def _make_model() -> MimicModel:
    return MimicModel(
        encoder=DeepSetsEncoder(
            element_encoder=MLPElementEncoder(8, 16, 16),
            aggregator=MeanAggregator(),
            rho=nn.Sequential(nn.Linear(16, 16), nn.ReLU()),
            output_dim=16,
        ),
        projector=MLPProjector(16, 8, 8),
        augmentation=Compose([FeatureNoise(std=0.1)]),
        loss_fn=NTXentLoss(temperature=0.1),
    )


class TestSaveLoad:
    def test_roundtrip(self, tmp_path) -> None:
        model1 = _make_model()
        path = tmp_path / "model.pt"
        model1.save(path)

        model2 = _make_model()
        model2.load(path)

        for (k1, v1), (k2, v2) in zip(
            model1.state_dict().items(), model2.state_dict().items()
        ):
            assert k1 == k2
            assert torch.equal(v1, v2), f"Mismatch at {k1}"

    def test_load_preserves_architecture(self, tmp_path) -> None:
        model = _make_model()
        model.train()
        x = torch.randn(2, 5, 8)

        path = tmp_path / "model.pt"
        model.save(path)

        model2 = _make_model()
        model2.load(path)
        model2.train()

        torch.manual_seed(42)
        emb1 = model.encode(x)
        torch.manual_seed(42)
        emb2 = model2.encode(x)
        assert torch.allclose(emb1, emb2)

    def test_save_creates_file(self, tmp_path) -> None:
        model = _make_model()
        path = tmp_path / "model.pt"
        assert not path.exists()
        model.save(path)
        assert path.exists()
        assert path.stat().st_size > 0
