"""Tests for NT-Xent loss."""

import torch

from mana.mimic.losses import NTXentLoss


class TestNTXentLoss:
    def test_identical_pairs_low_loss(self) -> None:
        """Identical projections should yield low loss."""
        loss_fn = NTXentLoss(temperature=0.1)
        z = torch.randn(8, 32)
        z = torch.nn.functional.normalize(z, dim=-1)
        loss = loss_fn(z, z)
        assert loss.item() < 1.0

    def test_random_pairs_higher_loss(self) -> None:
        """Random unrelated projections should yield higher loss."""
        loss_fn = NTXentLoss(temperature=0.1)
        z1 = torch.randn(8, 32)
        z2 = torch.randn(8, 32)
        z1 = torch.nn.functional.normalize(z1, dim=-1)
        z2 = torch.nn.functional.normalize(z2, dim=-1)
        loss = loss_fn(z1, z2)
        assert loss.item() > 1.0

    def test_loss_is_scalar(self) -> None:
        loss_fn = NTXentLoss()
        z1 = torch.randn(4, 16)
        z2 = torch.randn(4, 16)
        loss = loss_fn(z1, z2)
        assert loss.shape == ()

    def test_loss_is_differentiable(self) -> None:
        loss_fn = NTXentLoss()
        z1 = torch.randn(4, 16, requires_grad=True)
        z2 = torch.randn(4, 16, requires_grad=True)
        loss = loss_fn(z1, z2)
        loss.backward()
        assert z1.grad is not None
        assert z2.grad is not None

    def test_temperature_effect(self) -> None:
        """Lower temperature should produce larger loss magnitude for random pairs."""
        z1 = torch.randn(8, 32)
        z2 = torch.randn(8, 32)
        loss_low_t = NTXentLoss(temperature=0.05)(z1, z2)
        loss_high_t = NTXentLoss(temperature=1.0)(z1, z2)
        assert loss_low_t.item() > loss_high_t.item()


class TestLearnableTemperature:
    def test_learnable_has_parameter(self) -> None:
        loss_fn = NTXentLoss(temperature=0.1, learnable=True)
        param_names = [n for n, _ in loss_fn.named_parameters()]
        assert "_log_temp" in param_names

    def test_fixed_has_no_parameter(self) -> None:
        loss_fn = NTXentLoss(temperature=0.1, learnable=False)
        assert len(list(loss_fn.parameters())) == 0

    def test_temperature_has_gradient(self) -> None:
        loss_fn = NTXentLoss(temperature=0.1, learnable=True)
        z1 = torch.randn(4, 16)
        z2 = torch.randn(4, 16)
        loss = loss_fn(z1, z2)
        loss.backward()
        assert loss_fn._log_temp.grad is not None

    def test_learnable_backward_compatible(self) -> None:
        """Learnable and fixed should produce same loss for same temperature."""
        torch.manual_seed(0)
        z1 = torch.randn(8, 16)
        z2 = torch.randn(8, 16)
        loss_fixed = NTXentLoss(temperature=0.1, learnable=False)(z1, z2)
        loss_learn = NTXentLoss(temperature=0.1, learnable=True)(z1, z2)
        assert torch.allclose(loss_fixed, loss_learn, atol=1e-5)

    def test_temperature_clamped(self) -> None:
        """Temperature should stay within [0.01, 1.0]."""
        loss_fn = NTXentLoss(temperature=0.001, learnable=True)
        temp = loss_fn.temperature
        assert temp >= 0.01
        loss_fn2 = NTXentLoss(temperature=5.0, learnable=True)
        temp2 = loss_fn2.temperature
        assert temp2 <= 1.0
