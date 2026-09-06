"""Tests for regularizer loss functions."""

import torch

from mana.mimic.losses import covariance_loss, uniformity_loss, variance_loss


class TestUniformityLoss:
    def test_returns_scalar(self) -> None:
        z = torch.randn(8, 16)
        loss = uniformity_loss(z)
        assert loss.shape == ()

    def test_differentiable(self) -> None:
        z = torch.randn(8, 16, requires_grad=True)
        loss = uniformity_loss(z)
        loss.backward()
        assert z.grad is not None

    def test_clustered_higher_than_spread(self) -> None:
        torch.manual_seed(0)
        # Clustered: all points near a single direction
        base = torch.randn(1, 16)
        clustered = base.expand(16, -1) + torch.randn(16, 16) * 0.01

        # Spread: random directions on the sphere
        spread = torch.randn(16, 16)

        loss_clustered = uniformity_loss(clustered)
        loss_spread = uniformity_loss(spread)
        assert loss_clustered > loss_spread


class TestVarianceLoss:
    def test_returns_scalar(self) -> None:
        z = torch.randn(8, 16)
        loss = variance_loss(z)
        assert loss.shape == ()

    def test_differentiable(self) -> None:
        z = torch.randn(8, 16, requires_grad=True)
        loss = variance_loss(z)
        loss.backward()
        assert z.grad is not None

    def test_collapsed_high_loss(self) -> None:
        # All rows identical → std=0 per dim → loss = relu(1 - 0) = 1
        z = torch.ones(8, 16)
        loss = variance_loss(z)
        assert loss.item() > 0.9

    def test_spread_low_loss(self) -> None:
        # High variance per dim → std >> gamma → relu clipped to 0
        torch.manual_seed(0)
        z = torch.randn(64, 16) * 5.0
        loss = variance_loss(z, gamma=1.0)
        assert loss.item() < 0.1


class TestCovarianceLoss:
    def test_returns_scalar(self) -> None:
        z = torch.randn(8, 16)
        loss = covariance_loss(z)
        assert loss.shape == ()

    def test_differentiable(self) -> None:
        z = torch.randn(8, 16, requires_grad=True)
        loss = covariance_loss(z)
        loss.backward()
        assert z.grad is not None

    def test_correlated_high_loss(self) -> None:
        # All dimensions are copies of the first → high off-diagonal covariance
        torch.manual_seed(0)
        col = torch.randn(32, 1)
        z = col.expand(-1, 16) + torch.randn(32, 16) * 0.01
        loss = covariance_loss(z)
        assert loss.item() > 0.1

    def test_uncorrelated_low_loss(self) -> None:
        # Independent random dims → off-diagonal covariance ≈ 0
        torch.manual_seed(0)
        z = torch.randn(512, 16)
        loss = covariance_loss(z)
        assert loss.item() < 0.1
