"""ContrastiveMethod: two-view contrastive learning (SimCLR / MoCo / BYOL-lite)."""

from __future__ import annotations

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch import Tensor

from ..losses import covariance_loss, uniformity_loss, variance_loss
from ..types import Augmentation, ContrastiveLoss, Projector, SetEncoder
from .base import BaseMethod


def _masked_mean(x: Tensor, mask: Tensor | None) -> Tensor:
    """Compute mean over the set dimension, respecting the padding mask.

    Args:
        x: (B, N, D) input tensor.
        mask: (B, N) binary mask (1 = valid, 0 = padding), or None.

    Returns:
        (B, D) mean over valid elements.
    """
    if mask is not None:
        x = x * mask.unsqueeze(-1)
        counts = mask.sum(dim=1, keepdim=True).clamp(min=1)
        return x.sum(dim=1) / counts
    return x.mean(dim=1)


class ContrastiveMethod(BaseMethod):
    """Two-view contrastive learning for set-structured data.

    `training_step` creates two augmented views, encodes, projects, and
    returns the contrastive loss plus optional regularisers. An EMA encoder
    turns the symmetric SimCLR setup into a MoCo/BYOL-style asymmetric one.

    This is the original `MimicModel` objective, now expressed as one point on
    axis B alongside masked autoencoding, redundancy reduction, and others.
    """

    def __init__(
        self,
        encoder: SetEncoder,
        projector: Projector,
        augmentation: Augmentation,
        loss_fn: ContrastiveLoss,
        decoder: nn.Module | None = None,
        reconstruction_weight: float = 0.1,
        uniformity_weight: float = 0.0,
        variance_weight: float = 0.0,
        covariance_weight: float = 0.0,
        ema_encoder: nn.Module | None = None,
    ) -> None:
        super().__init__()
        self.encoder = encoder
        self.projector = projector
        self.augmentation = augmentation
        self.loss_fn = loss_fn
        self.decoder = decoder
        self.reconstruction_weight = reconstruction_weight
        self.uniformity_weight = uniformity_weight
        self.variance_weight = variance_weight
        self.covariance_weight = covariance_weight
        self.ema_encoder = ema_encoder

    def training_step(
        self, x: Tensor, mask: Tensor | None = None, labels: Tensor | None = None
    ) -> Tensor:
        """Create augmented views, encode, project, compute loss.

        labels: optional (B,) integer labels for supervised contrastive losses.
        """
        x1, m1 = self.augmentation(x, mask)
        x2, m2 = self.augmentation(x, mask)
        h1 = self.encoder(x1, m1)
        if self.ema_encoder is not None:
            h2 = self.ema_encoder(x2, m2).detach()
        else:
            h2 = self.encoder(x2, m2)
        z1 = self.projector(h1)
        z2 = self.projector(h2)
        loss = self.loss_fn(z1, z2, labels)

        if self.decoder is not None and self.reconstruction_weight > 0:
            recon_target = _masked_mean(x, mask).detach()
            recon_loss = 0.5 * (
                F.mse_loss(self.decoder(h1), recon_target)
                + F.mse_loss(self.decoder(h2), recon_target)
            )
            loss = loss + self.reconstruction_weight * recon_loss

        if self.uniformity_weight > 0:
            loss = loss + self.uniformity_weight * 0.5 * (
                uniformity_loss(z1) + uniformity_loss(z2)
            )

        if self.variance_weight > 0:
            loss = loss + self.variance_weight * 0.5 * (
                variance_loss(h1) + variance_loss(h2)
            )

        if self.covariance_weight > 0:
            loss = loss + self.covariance_weight * 0.5 * (
                covariance_loss(h1) + covariance_loss(h2)
            )

        return loss

    @torch.no_grad()
    def embed(self, x: Tensor, mask: Tensor | None = None) -> Tensor:
        """Encode and project sets without augmentation. Returns projected embeddings."""
        was_training = self.training
        self.set_eval_mode()
        h = self.projector(self.encoder(x, mask))
        if was_training:
            self.train()
        return h
