"""Tests for mixed precision (AMP) training."""

import torch
import torch.nn as nn

from mana.mimic import (
    Compose,
    ContrastiveTrainer,
    DeepSetsEncoder,
    FeatureNoise,
    MeanAggregator,
    MLPElementEncoder,
    MLPProjector,
    MimicModel,
    NTXentLoss,
    SetDataset,
    TrainConfig,
    set_dataloader,
)


def _make_pipeline(seed: int = 42):
    torch.manual_seed(seed)
    data = [torch.randn(torch.randint(3, 8, (1,)).item(), 4) for _ in range(16)]
    ds = SetDataset(data)
    dl = set_dataloader(ds, batch_size=8, shuffle=True)

    model = MimicModel(
        encoder=DeepSetsEncoder(
            element_encoder=MLPElementEncoder(4, 16, 16),
            aggregator=MeanAggregator(),
            rho=nn.Sequential(nn.Linear(16, 16), nn.ReLU()),
            output_dim=16,
        ),
        projector=MLPProjector(16, 8, 8),
        augmentation=Compose([FeatureNoise(std=0.1)]),
        loss_fn=NTXentLoss(temperature=0.1),
    )
    return model, dl


class TestAMP:
    def test_amp_training_completes(self) -> None:
        """AMP on CPU is a no-op but should not crash."""
        model, dl = _make_pipeline()
        config = TrainConfig(epochs=5, lr=1e-3, verbose=False, amp=True)
        result = ContrastiveTrainer(model, config).fit(dl)
        assert len(result.epoch_losses) == 5
        assert all(torch.isfinite(torch.tensor(loss)) for loss in result.epoch_losses)

    def test_amp_with_grad_clipping(self) -> None:
        """AMP + grad clipping should work together (scaler.unscale_ before clip)."""
        model, dl = _make_pipeline()
        config = TrainConfig(
            epochs=5, lr=1e-3, verbose=False, amp=True, max_grad_norm=1.0
        )
        result = ContrastiveTrainer(model, config).fit(dl)
        assert len(result.epoch_losses) == 5

    def test_amp_false_unchanged(self) -> None:
        """With amp=False, behavior should match original training."""
        model, dl = _make_pipeline(seed=99)
        config = TrainConfig(epochs=5, lr=1e-3, verbose=False, amp=False)
        result = ContrastiveTrainer(model, config).fit(dl)
        assert len(result.epoch_losses) == 5
        # Loss should decrease
        assert result.epoch_losses[-1] < result.epoch_losses[0]

    def test_amp_with_early_stopping(self) -> None:
        """AMP + early stopping should work together."""
        model, dl = _make_pipeline()
        config = TrainConfig(epochs=100, lr=1e-7, verbose=False, amp=True, patience=3)
        result = ContrastiveTrainer(model, config).fit(dl)
        assert result.stopped_early is True
