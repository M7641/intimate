from __future__ import annotations

import math
from typing import Callable, Literal

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch import Tensor


class SupConLoss(nn.Module):
    """Supervised Contrastive Loss.

    When labels are provided, all samples with the same label form positive
    pairs (not just augmented views of the same instance). When labels are
    None, falls back to standard self-supervised NT-Xent behaviour where
    only (z1[i], z2[i]) are positive pairs.
    """

    def __init__(self, temperature: float = 0.1) -> None:
        super().__init__()
        self.temperature = temperature

    def forward(self, z1: Tensor, z2: Tensor, labels: Tensor | None = None) -> Tensor:
        batch_size = z1.shape[0]
        device = z1.device

        z = torch.cat([z1, z2], dim=0)  # (2B, D)
        z = F.normalize(z, dim=-1)
        sim = (z @ z.T) / self.temperature  # (2B, 2B)

        # Self-similarity mask (diagonal)
        self_mask = torch.eye(2 * batch_size, dtype=torch.bool, device=device)

        if labels is not None:
            # Supervised: all same-label pairs are positive
            labels_2b = torch.cat([labels, labels])  # (2B,)
            pos_mask = (labels_2b.unsqueeze(0) == labels_2b.unsqueeze(1)) & ~self_mask
        else:
            # Self-supervised fallback: only augmented pairs are positive
            pos_mask = torch.zeros(
                2 * batch_size, 2 * batch_size, dtype=torch.bool, device=device
            )
            pos_mask[
                torch.arange(batch_size), torch.arange(batch_size, 2 * batch_size)
            ] = True
            pos_mask[
                torch.arange(batch_size, 2 * batch_size), torch.arange(batch_size)
            ] = True

        # Mask diagonal from logits
        sim = sim.masked_fill(self_mask, float("-inf"))

        # Log-softmax over all non-self pairs
        log_prob = sim - torch.logsumexp(sim, dim=1, keepdim=True)

        # Zero out diagonal to avoid -inf * 0 = NaN
        log_prob = log_prob.masked_fill(self_mask, 0.0)

        # Mean log-probability over positive pairs
        num_positives = pos_mask.sum(dim=1).clamp(min=1)
        mean_log_prob = (log_prob * pos_mask.float()).sum(dim=1) / num_positives

        return -mean_log_prob.mean()


def _cosine_similarity(z1: Tensor, z2: Tensor) -> Tensor:
    """Pairwise cosine similarity matrix."""
    z1 = torch.nn.functional.normalize(z1, dim=-1)
    z2 = torch.nn.functional.normalize(z2, dim=-1)
    return z1 @ z2.T


class NTXentLoss(nn.Module):
    """NT-Xent contrastive loss with optional hard negative mining.

    Given two batches of projections z1, z2 (each of shape (B, D)),
    computes the InfoNCE-style contrastive loss where positive pairs
    are (z1[i], z2[i]) and all other pairs are negatives.

    Strategies:
    - "none": Standard NT-Xent (SimCLR). All negatives weighted equally.
    - "debiased": Corrects sampling bias in finite batches (Chuang et al., 2020).
    - "hardest": Uses only the single hardest negative per anchor.
    - "semi-hard": Uses negatives harder than the positive but within a margin.

    When learnable=True, temperature is an nn.Parameter optimised via
    gradient descent (log-space parameterisation, clamped to [0.01, 1.0]).
    """

    def __init__(
        self,
        # TUNING: temperature is the single most sensitive hyperparameter.
        # Lower values (0.05) sharpen the softmax — the model focuses on the
        # hardest negatives but gradients can explode. Higher values (0.5)
        # smooth it out — easier to optimise but weaker discrimination.
        # Start at 0.1, lower to 0.07 if loss plateaus, raise if training
        # diverges. Consider learnable=True to let the model find its own.
        temperature: float = 0.1,
        # TUNING: "debiased" helps with small batch sizes (<64) by correcting
        # the sampling bias from having few negatives. "hardest" and "semi-hard"
        # can improve late-stage training but risk collapse if used from the start.
        strategy: Literal["none", "debiased", "hardest", "semi-hard"] = "none",
        tau_plus: float = 0.1,
        beta: float = 1.0,
        margin: float = 0.1,
        similarity_fn: Callable[[Tensor, Tensor], Tensor] | None = None,
        learnable: bool = False,
    ) -> None:
        super().__init__()
        self.strategy = strategy
        self.tau_plus = tau_plus
        self.beta = beta
        self.margin = margin
        self.similarity_fn = similarity_fn or _cosine_similarity
        self.learnable = learnable

        if learnable:
            self._log_temp = nn.Parameter(torch.tensor(math.log(temperature)))
        else:
            self._fixed_temp = temperature

    @property
    def temperature(self) -> float | Tensor:
        if self.learnable:
            return self._log_temp.exp().clamp(min=0.01, max=1.0)
        return self._fixed_temp

    def forward(self, z1: Tensor, z2: Tensor, labels: Tensor | None = None) -> Tensor:
        if self.strategy == "none":
            return self._standard(z1, z2)
        elif self.strategy == "debiased":
            return self._debiased(z1, z2)
        elif self.strategy == "hardest":
            return self._hardest(z1, z2)
        else:
            return self._semi_hard(z1, z2)

    def _standard(self, z1: Tensor, z2: Tensor) -> Tensor:
        """Standard NT-Xent (SimCLR)."""
        batch_size = z1.shape[0]
        device = z1.device

        z = torch.cat([z1, z2], dim=0)  # (2B, D)
        sim = self.similarity_fn(z, z) / self.temperature  # (2B, 2B)

        # Mask out self-similarity (diagonal)
        mask = ~torch.eye(2 * batch_size, dtype=torch.bool, device=device)
        sim = sim.masked_fill(~mask, float("-inf"))

        # Positive pairs: (i, i+B) and (i+B, i)
        targets = torch.cat(
            [
                torch.arange(batch_size, 2 * batch_size, device=device),
                torch.arange(batch_size, device=device),
            ]
        )

        return F.cross_entropy(sim, targets)

    def _debiased(self, z1: Tensor, z2: Tensor) -> Tensor:
        """Debiased contrastive loss (Chuang et al., 2020).

        Computed entirely in log-space to avoid exp() overflow at low
        temperatures and uses a fixed numerical floor instead of a
        temperature-dependent clamp.
        """
        batch_size = z1.shape[0]
        device = z1.device

        z = torch.cat([z1, z2], dim=0)
        sim = self.similarity_fn(z, z) / self.temperature

        self_mask = torch.eye(2 * batch_size, dtype=torch.bool, device=device)

        pos_mask = torch.zeros(
            2 * batch_size, 2 * batch_size, dtype=torch.bool, device=device
        )
        pos_mask[torch.arange(batch_size), torch.arange(batch_size, 2 * batch_size)] = (
            True
        )
        pos_mask[torch.arange(batch_size, 2 * batch_size), torch.arange(batch_size)] = (
            True
        )

        neg_mask = ~(self_mask | pos_mask)

        # pos_sim: (2B,) — one positive similarity per anchor
        pos_sim = (sim * pos_mask.float()).sum(dim=1)

        # log-sum of raw negative exponentials (numerically stable)
        neg_sim = sim.masked_fill(~neg_mask, float("-inf"))
        log_neg_sum = torch.logsumexp(neg_sim, dim=1)  # log(sum(exp(neg)))

        # log of the bias correction term: log(tau_plus * N * exp(pos_sim))
        N = neg_mask.sum(dim=1).float()
        log_correction = torch.log(self.tau_plus * N) + pos_sim

        # log(neg_debiased) = log((sum_neg - correction) / (1 - tau_plus))
        # = log(exp(log_neg_sum) - exp(log_correction)) - log(1 - tau_plus)
        # Use log-subtract: log(a - b) = log_a + log1p(-exp(log_b - log_a))
        diff = log_correction - log_neg_sum
        log_neg_debiased = (
            log_neg_sum
            + torch.log1p(-diff.exp().clamp(max=1.0 - 1e-7))
            - math.log(1.0 - self.tau_plus)
        )

        # Fixed numerical floor (not temperature-dependent)
        _LOG_FLOOR = math.log(1e-8)
        log_neg_debiased = log_neg_debiased.clamp(min=_LOG_FLOOR)

        # loss = -pos + log(exp(pos) + exp(log_neg_debiased))
        loss = -pos_sim + torch.logaddexp(pos_sim, log_neg_debiased)
        return loss.mean()

    def _hardest(self, z1: Tensor, z2: Tensor) -> Tensor:
        """Use only the single hardest negative per anchor."""
        batch_size = z1.shape[0]
        device = z1.device

        z = torch.cat([z1, z2], dim=0)
        sim = self.similarity_fn(z, z) / self.temperature

        self_mask = torch.eye(2 * batch_size, dtype=torch.bool, device=device)
        pos_mask = torch.zeros(
            2 * batch_size, 2 * batch_size, dtype=torch.bool, device=device
        )
        pos_mask[torch.arange(batch_size), torch.arange(batch_size, 2 * batch_size)] = (
            True
        )
        pos_mask[torch.arange(batch_size, 2 * batch_size), torch.arange(batch_size)] = (
            True
        )

        neg_mask = ~(self_mask | pos_mask)

        pos_sim = (sim * pos_mask.float()).sum(dim=1)

        neg_sim = sim.masked_fill(~neg_mask, float("-inf"))
        hardest_neg, _ = neg_sim.max(dim=1)

        loss = -pos_sim + torch.logsumexp(
            torch.stack([pos_sim, hardest_neg], dim=1), dim=1
        )
        return loss.mean()

    def _semi_hard(self, z1: Tensor, z2: Tensor) -> Tensor:
        """Use negatives harder than positive but within a margin."""
        batch_size = z1.shape[0]
        device = z1.device

        z = torch.cat([z1, z2], dim=0)
        sim = self.similarity_fn(z, z) / self.temperature

        self_mask = torch.eye(2 * batch_size, dtype=torch.bool, device=device)
        pos_mask = torch.zeros(
            2 * batch_size, 2 * batch_size, dtype=torch.bool, device=device
        )
        pos_mask[torch.arange(batch_size), torch.arange(batch_size, 2 * batch_size)] = (
            True
        )
        pos_mask[torch.arange(batch_size, 2 * batch_size), torch.arange(batch_size)] = (
            True
        )

        neg_mask = ~(self_mask | pos_mask)

        pos_sim = (sim * pos_mask.float()).sum(dim=1)

        threshold = (pos_sim - self.margin).unsqueeze(1)
        semi_hard_mask = neg_mask & (sim > threshold)

        has_semi_hard = semi_hard_mask.any(dim=1)
        effective_mask = torch.where(
            has_semi_hard.unsqueeze(1), semi_hard_mask, neg_mask
        )

        neg_sim_filtered = sim.masked_fill(~effective_mask, float("-inf"))
        log_neg = torch.logsumexp(neg_sim_filtered, dim=1)

        loss = -pos_sim + torch.logsumexp(torch.stack([pos_sim, log_neg], dim=1), dim=1)
        return loss.mean()

    def _temp_value(self) -> float:
        """Get temperature as a plain float (for math operations)."""
        if self.learnable:
            return self._log_temp.exp().clamp(min=0.01, max=1.0).item()
        return self._fixed_temp


def uniformity_loss(z: Tensor, t: float = 2.0) -> Tensor:
    """Differentiable uniformity loss (Wang & Isola, 2020)."""
    z = F.normalize(z, dim=-1)
    sq_dists = torch.cdist(z, z, p=2.0).pow(2)
    n = z.shape[0]
    mask = ~torch.eye(n, dtype=torch.bool, device=z.device)
    kernel = (-t * sq_dists).masked_fill(~mask, float("-inf"))
    return torch.logsumexp(kernel.view(-1), dim=0) - math.log(n * (n - 1))


def variance_loss(z: Tensor, gamma: float = 1.0) -> Tensor:
    """Variance hinge regularizer (VICReg)."""
    return F.relu(gamma - z.std(dim=0)).mean()


def covariance_loss(z: Tensor) -> Tensor:
    """Off-diagonal covariance regularizer (VICReg)."""
    z = z - z.mean(dim=0)
    n = z.shape[0]
    cov = (z.T @ z) / max(n - 1, 1)
    d = cov.shape[0]
    off_diag = cov.pow(2).sum() - cov.diagonal().pow(2).sum()
    return off_diag / d
