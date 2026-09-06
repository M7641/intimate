"""Tests for hard negative mining strategies."""

import torch
import torch.nn.functional as F

from mana.mimic.losses import NTXentLoss


class TestHardNegativeStrategies:
    def test_debiased_returns_scalar(self) -> None:
        loss_fn = NTXentLoss(strategy="debiased")
        z1, z2 = torch.randn(8, 16), torch.randn(8, 16)
        loss = loss_fn(z1, z2)
        assert loss.shape == ()

    def test_hardest_returns_scalar(self) -> None:
        loss_fn = NTXentLoss(strategy="hardest")
        z1, z2 = torch.randn(8, 16), torch.randn(8, 16)
        loss = loss_fn(z1, z2)
        assert loss.shape == ()

    def test_semi_hard_returns_scalar(self) -> None:
        loss_fn = NTXentLoss(strategy="semi-hard")
        z1, z2 = torch.randn(8, 16), torch.randn(8, 16)
        loss = loss_fn(z1, z2)
        assert loss.shape == ()

    def test_all_strategies_differentiable(self) -> None:
        for strategy in ["none", "debiased", "hardest", "semi-hard"]:
            loss_fn = NTXentLoss(strategy=strategy)
            z1 = torch.randn(8, 16, requires_grad=True)
            z2 = torch.randn(8, 16, requires_grad=True)
            loss = loss_fn(z1, z2)
            loss.backward()
            assert z1.grad is not None
            assert z2.grad is not None

    def test_identical_pairs_low_loss(self) -> None:
        """Identical projections should yield lower loss than random pairs."""
        for strategy in ["none", "debiased", "hardest", "semi-hard"]:
            loss_fn = NTXentLoss(temperature=0.1, strategy=strategy)
            z = F.normalize(torch.randn(8, 32), dim=-1)
            loss_identical = loss_fn(z, z).item()

            z1 = F.normalize(torch.randn(8, 32), dim=-1)
            z2 = F.normalize(torch.randn(8, 32), dim=-1)
            loss_random = loss_fn(z1, z2).item()
            assert loss_identical < loss_random, (
                f"{strategy}: identical should be < random"
            )

    def test_debiased_no_overflow_low_temperature(self) -> None:
        """At very low temperature, debiased loss must remain finite (no exp overflow)."""
        loss_fn = NTXentLoss(temperature=0.01, strategy="debiased")
        z1 = F.normalize(torch.randn(8, 16), dim=-1)
        z2 = F.normalize(torch.randn(8, 16), dim=-1)
        loss = loss_fn(z1, z2)
        assert torch.isfinite(loss), f"Loss is not finite: {loss.item()}"

    def test_debiased_correctness_manual(self) -> None:
        """Manual log-space reference computation on a tiny batch at t=0.5."""
        torch.manual_seed(123)
        t = 0.5
        tau_plus = 0.1
        B = 4
        D = 8
        z1 = F.normalize(torch.randn(B, D), dim=-1)
        z2 = F.normalize(torch.randn(B, D), dim=-1)

        # Reference computation in log-space
        z = torch.cat([z1, z2], dim=0)
        z_norm = F.normalize(z, dim=-1)
        sim = (z_norm @ z_norm.T) / t

        N_total = 2 * B
        self_mask = torch.eye(N_total, dtype=torch.bool)
        pos_mask = torch.zeros(N_total, N_total, dtype=torch.bool)
        pos_mask[torch.arange(B), torch.arange(B, N_total)] = True
        pos_mask[torch.arange(B, N_total), torch.arange(B)] = True
        neg_mask = ~(self_mask | pos_mask)

        pos_sim = (sim * pos_mask.float()).sum(dim=1)
        neg_sim = sim.masked_fill(~neg_mask, float("-inf"))
        log_neg_sum = torch.logsumexp(neg_sim, dim=1)

        N = neg_mask.sum(dim=1).float()
        import math

        log_correction = math.log(tau_plus) + torch.log(N) + pos_sim
        diff = log_correction - log_neg_sum
        log_neg_debiased = (
            log_neg_sum
            + torch.log1p(-diff.exp().clamp(max=1.0 - 1e-7))
            - math.log(1.0 - tau_plus)
        )
        log_neg_debiased = log_neg_debiased.clamp(min=math.log(1e-8))

        ref_loss = (-pos_sim + torch.logaddexp(pos_sim, log_neg_debiased)).mean()

        # Actual
        loss_fn = NTXentLoss(temperature=t, strategy="debiased", tau_plus=tau_plus)
        actual_loss = loss_fn(z1, z2)

        assert torch.allclose(actual_loss, ref_loss, atol=1e-5), (
            f"Mismatch: actual={actual_loss.item():.6f}, ref={ref_loss.item():.6f}"
        )

    def test_debiased_leq_standard(self) -> None:
        """Debiased loss should generally be <= standard NT-Xent on same inputs."""
        torch.manual_seed(42)
        z1 = F.normalize(torch.randn(16, 32), dim=-1)
        z2 = F.normalize(torch.randn(16, 32), dim=-1)

        standard = NTXentLoss(temperature=0.1, strategy="none")(z1, z2).item()
        debiased = NTXentLoss(temperature=0.1, strategy="debiased")(z1, z2).item()
        assert debiased < standard + 0.5
