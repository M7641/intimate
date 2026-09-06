"""Tests for Supervised Contrastive Loss."""

import torch
import torch.nn.functional as F

from mana.mimic.losses import SupConLoss


class TestSupConLoss:
    def test_returns_scalar(self) -> None:
        loss_fn = SupConLoss()
        z1, z2 = torch.randn(8, 16), torch.randn(8, 16)
        labels = torch.tensor([0, 0, 1, 1, 2, 2, 3, 3])
        loss = loss_fn(z1, z2, labels)
        assert loss.shape == ()

    def test_differentiable(self) -> None:
        loss_fn = SupConLoss()
        z1 = torch.randn(8, 16, requires_grad=True)
        z2 = torch.randn(8, 16, requires_grad=True)
        labels = torch.tensor([0, 0, 1, 1, 2, 2, 3, 3])
        loss = loss_fn(z1, z2, labels)
        loss.backward()
        assert z1.grad is not None

    def test_self_supervised_fallback(self) -> None:
        """Without labels, behaves like NT-Xent."""
        loss_fn = SupConLoss(temperature=0.1)
        z = F.normalize(torch.randn(8, 16), dim=-1)
        loss = loss_fn(z, z, labels=None)
        # Identical pairs should give low loss
        assert loss.item() < 1.0

    def test_labels_affect_loss(self) -> None:
        """Different label assignments should produce different losses."""
        torch.manual_seed(42)
        z1 = F.normalize(torch.randn(8, 16), dim=-1)
        z2 = F.normalize(torch.randn(8, 16), dim=-1)
        loss_all_same = SupConLoss()(z1, z2, torch.zeros(8).long()).item()
        loss_all_diff = SupConLoss()(z1, z2, torch.arange(8).long()).item()
        # With different label assignments, loss values should differ
        assert loss_all_same != loss_all_diff
