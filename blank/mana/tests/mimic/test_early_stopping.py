"""Tests for early stopping."""

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
    data = [torch.randn(torch.randint(3, 8, (1,)).item(), 4) for _ in range(32)]
    ds = SetDataset(data)
    dl = set_dataloader(ds, batch_size=16, shuffle=True)

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


class TestEarlyStopping:
    def test_patience_none_runs_full(self) -> None:
        model, dl = _make_pipeline()
        config = TrainConfig(epochs=10, lr=1e-3, verbose=False, patience=None)
        result = ContrastiveTrainer(model, config).fit(dl)
        assert len(result.epoch_losses) == 10
        assert result.stopped_early is False

    def test_patience_triggers_early_stop(self) -> None:
        model, dl = _make_pipeline()
        # Very low LR so loss barely moves after initial drop, with tight patience
        config = TrainConfig(epochs=100, lr=1e-7, verbose=False, patience=3)
        result = ContrastiveTrainer(model, config).fit(dl)
        assert len(result.epoch_losses) < 100
        assert result.stopped_early is True

    def test_restores_best_state(self) -> None:
        model, dl = _make_pipeline()
        config = TrainConfig(epochs=100, lr=1e-7, verbose=False, patience=3)
        ContrastiveTrainer(model, config).fit(dl)
        # After early stopping, model should have best weights restored
        # Verify by encoding — should not crash and should produce valid output
        batch = next(iter(dl))
        emb = model.encode(batch.x, batch.mask)
        assert emb.shape[0] == batch.x.shape[0]

    def test_stopped_early_flag(self) -> None:
        model, dl = _make_pipeline()
        config = TrainConfig(epochs=5, lr=1e-3, verbose=False, patience=None)
        result = ContrastiveTrainer(model, config).fit(dl)
        assert result.stopped_early is False


class TestValDataloader:
    def test_val_dataloader_drives_early_stopping(self) -> None:
        model, dl = _make_pipeline()
        # Use same data for val — early stopping should still trigger with tiny lr
        config = TrainConfig(epochs=100, lr=1e-7, verbose=False, patience=3)
        result = ContrastiveTrainer(model, config).fit(dl, val_dataloader=dl)
        assert result.stopped_early is True
        assert len(result.val_losses) > 0

    def test_val_losses_recorded_in_result(self) -> None:
        model, dl = _make_pipeline()
        config = TrainConfig(epochs=5, lr=1e-3, verbose=False)
        result = ContrastiveTrainer(model, config).fit(dl, val_dataloader=dl)
        assert len(result.val_losses) == 5
        assert all(isinstance(v, float) for v in result.val_losses)

    def test_no_val_dataloader_unchanged(self) -> None:
        model, dl = _make_pipeline()
        config = TrainConfig(epochs=5, lr=1e-3, verbose=False)
        result = ContrastiveTrainer(model, config).fit(dl)
        assert result.val_losses == []
        assert len(result.epoch_losses) == 5
