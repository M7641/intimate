"""Tests for EMAEncoder."""

import math

import torch
import torch.nn as nn

from mana.mimic import (
    Compose,
    ContrastiveTrainer,
    DeepSetsEncoder,
    EMAEncoder,
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


def _make_encoder():
    return DeepSetsEncoder(
        element_encoder=MLPElementEncoder(4, 16, 16),
        aggregator=MeanAggregator(),
        rho=nn.Sequential(nn.Linear(16, 8), nn.ReLU()),
        output_dim=8,
    )


class TestEMAEncoder:
    def test_shadow_no_grad(self) -> None:
        encoder = _make_encoder()
        ema = EMAEncoder(encoder, momentum=0.99)
        for p in ema.shadow.parameters():
            assert not p.requires_grad

    def test_shadow_starts_as_copy(self) -> None:
        encoder = _make_encoder()
        ema = EMAEncoder(encoder, momentum=0.99)
        for sp, mp in zip(ema.shadow.parameters(), encoder.parameters()):
            assert torch.equal(sp.data, mp.data)

    def test_update_moves_weights(self) -> None:
        encoder = _make_encoder()
        ema = EMAEncoder(encoder, momentum=0.9)

        # Snapshot shadow before update
        before = {k: v.clone() for k, v in ema.shadow.state_dict().items()}

        # Modify encoder weights
        with torch.no_grad():
            for p in encoder.parameters():
                p.add_(torch.randn_like(p) * 0.5)

        ema.update(encoder)

        # Shadow should have moved toward encoder
        for k in before:
            shadow_now = ema.shadow.state_dict()[k]
            if shadow_now.is_floating_point():
                assert not torch.equal(shadow_now, before[k]), (
                    f"Shadow didn't move at {k}"
                )

    def test_momentum_formula(self) -> None:
        encoder = nn.Linear(4, 4)
        ema = EMAEncoder(encoder, momentum=0.5, cosine_schedule=False)

        shadow_before = ema.shadow.weight.data.clone()

        with torch.no_grad():
            encoder.weight.fill_(1.0)

        ema.update(encoder)

        expected = 0.5 * shadow_before + 0.5 * torch.ones_like(shadow_before)
        assert torch.allclose(ema.shadow.weight.data, expected)

    def test_cosine_schedule(self) -> None:
        encoder = _make_encoder()
        ema = EMAEncoder(encoder, momentum=0.996, cosine_schedule=True)

        # At progress=0: m = 1 - (1-0.996)(1+cos(0))/2 = 1 - 0.004*1 = 0.996
        ema.set_progress(0.0)
        assert abs(ema._current_momentum - 0.996) < 1e-6

        # At progress=1: m = 1 - (1-0.996)(1+cos(pi))/2 = 1 - 0.004*0 = 1.0
        ema.set_progress(1.0)
        assert abs(ema._current_momentum - 1.0) < 1e-6

        # At progress=0.5: m = 1 - (1-0.996)(1+cos(pi/2))/2 = 1 - 0.004*0.5 = 0.998
        ema.set_progress(0.5)
        expected = 1.0 - (1.0 - 0.996) * (1.0 + math.cos(math.pi * 0.5)) / 2.0
        assert abs(ema._current_momentum - expected) < 1e-6

    def test_fixed_momentum_no_schedule(self) -> None:
        encoder = _make_encoder()
        ema = EMAEncoder(encoder, momentum=0.99, cosine_schedule=False)
        ema.set_progress(0.5)
        assert ema._current_momentum == 0.99

    def test_forward_uses_shadow(self) -> None:
        encoder = _make_encoder()
        ema = EMAEncoder(encoder, momentum=0.99)

        # Mutate encoder so shadow != encoder
        with torch.no_grad():
            for p in encoder.parameters():
                p.zero_()

        x = torch.randn(2, 5, 4)
        out_ema = ema(x)
        out_encoder = encoder(x)

        # They should differ since encoder is zeroed but shadow has original weights
        assert not torch.allclose(out_ema, out_encoder, atol=1e-3)


class TestEMAIntegration:
    def test_asymmetric_forward(self) -> None:
        """MimicModel with ema_encoder should use EMA for view 2."""
        encoder = _make_encoder()
        ema = EMAEncoder(encoder, momentum=0.99)

        model = MimicModel(
            encoder=encoder,
            projector=MLPProjector(8, 8, 8),
            augmentation=Compose([FeatureNoise(std=0.1)]),
            loss_fn=NTXentLoss(temperature=0.1),
            ema_encoder=ema,
        )
        model.train()
        x = torch.randn(4, 5, 4)
        mask = torch.ones(4, 5, dtype=torch.bool)
        loss = model(x, mask)
        assert loss.shape == ()
        assert loss.requires_grad

    def test_ema_detach_no_gradient(self) -> None:
        """Gradients should not flow through the EMA encoder path."""
        encoder = _make_encoder()
        ema = EMAEncoder(encoder, momentum=0.99)

        model = MimicModel(
            encoder=encoder,
            projector=MLPProjector(8, 8, 8),
            augmentation=Compose([FeatureNoise(std=0.1)]),
            loss_fn=NTXentLoss(temperature=0.1),
            ema_encoder=ema,
        )
        model.train()
        x = torch.randn(4, 5, 4)
        loss = model(x)
        loss.backward()

        # EMA shadow params should have no gradients
        for p in ema.shadow.parameters():
            assert p.grad is None

    def test_full_training_with_ema(self) -> None:
        torch.manual_seed(42)
        data = [torch.randn(torch.randint(3, 8, (1,)).item(), 4) for _ in range(32)]
        ds = SetDataset(data)
        dl = set_dataloader(ds, batch_size=16, shuffle=True)

        encoder = DeepSetsEncoder(
            element_encoder=MLPElementEncoder(4, 16, 16),
            aggregator=MeanAggregator(),
            rho=nn.Sequential(nn.Linear(16, 16), nn.ReLU()),
            output_dim=16,
        )
        ema = EMAEncoder(encoder, momentum=0.996)

        model = MimicModel(
            encoder=encoder,
            projector=MLPProjector(16, 8, 8),
            augmentation=Compose([FeatureNoise(std=0.1)]),
            loss_fn=NTXentLoss(temperature=0.1),
            ema_encoder=ema,
        )

        config = TrainConfig(epochs=10, lr=1e-3, verbose=False)
        result = ContrastiveTrainer(model, config).fit(dl)
        assert len(result.epoch_losses) == 10
        assert result.epoch_losses[-1] < result.epoch_losses[0]

    def test_ema_weights_diverge_from_encoder(self) -> None:
        """After training, shadow should differ from encoder."""
        torch.manual_seed(42)
        data = [torch.randn(torch.randint(3, 6, (1,)).item(), 4) for _ in range(16)]
        ds = SetDataset(data)
        dl = set_dataloader(ds, batch_size=8, shuffle=True)

        encoder = DeepSetsEncoder(
            element_encoder=MLPElementEncoder(4, 16, 16),
            aggregator=MeanAggregator(),
            rho=nn.Sequential(nn.Linear(16, 8), nn.ReLU()),
            output_dim=8,
        )
        ema = EMAEncoder(encoder, momentum=0.9)

        model = MimicModel(
            encoder=encoder,
            projector=MLPProjector(8, 8, 8),
            augmentation=Compose([FeatureNoise(std=0.1)]),
            loss_fn=NTXentLoss(temperature=0.1),
            ema_encoder=ema,
        )

        ContrastiveTrainer(model, TrainConfig(epochs=5, lr=1e-2, verbose=False)).fit(dl)

        # Shadow and encoder should differ (momentum < 1)
        any_diff = False
        for sp, ep in zip(ema.shadow.parameters(), encoder.parameters()):
            if not torch.equal(sp.data, ep.data):
                any_diff = True
                break
        assert any_diff
