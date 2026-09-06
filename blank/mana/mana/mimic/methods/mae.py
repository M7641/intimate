"""MaskedSetAutoencoder: a non-contrastive, masking-based objective (axis B)."""

from __future__ import annotations

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch import Tensor

from ..types import SetEncoder
from .base import BaseMethod
from .contrastive import _masked_mean


class MaskedSetAutoencoder(BaseMethod):
    """Masked set autoencoding — the simplest demonstration that the Method
    abstraction generalises beyond the two-view contrastive mould.

    Pipeline: mask out a fraction of a set's elements → encode the *partial*
    set → reconstruct a summary of the *full* set. There is no second view, no
    projector, and no negatives. The training signal is "infer the whole from a
    part", which forces the encoder to capture set-level structure.

    Because a `SetEncoder` pools a set down to one vector, the reconstruction
    target is set-level statistics (mean ⊕ variance) rather than per-element
    features. Per-element reconstruction would need a token-level (non-pooling)
    encoder — a natural follow-up once such an encoder exists on axis A.

    This objective is well suited to heterogeneous tabular sets, where
    designing plausible contrastive augmentations is awkward: masking sidesteps
    that question entirely.
    """

    def __init__(
        self,
        encoder: SetEncoder,
        decoder: nn.Module,
        mask_ratio: float = 0.5,
    ) -> None:
        super().__init__()
        self.encoder = encoder
        self.decoder = decoder
        self.mask_ratio = mask_ratio

    def training_step(
        self, x: Tensor, mask: Tensor | None = None, labels: Tensor | None = None
    ) -> Tensor:
        target = self._summary(x, mask).detach()
        corrupt_mask = self._corrupt(x, mask)
        h = self.encoder(x, corrupt_mask)
        pred = self.decoder(h)
        return F.mse_loss(pred, target)

    def _summary(self, x: Tensor, mask: Tensor | None) -> Tensor:
        """Mean ⊕ variance of the set's element features. Shape (B, 2*D)."""
        mean = _masked_mean(x, mask)
        centered = x - mean.unsqueeze(1)
        var = _masked_mean(centered * centered, mask)
        return torch.cat([mean, var], dim=-1)

    def _corrupt(self, x: Tensor, mask: Tensor | None) -> Tensor:
        """Drop a fraction of valid elements by masking them out of the set.

        Returns a new padding mask; at least one element per row survives so the
        encoder always sees a non-empty set.
        """
        if mask is None:
            mask = torch.ones(x.shape[:2], dtype=torch.bool, device=x.device)

        rand = torch.rand(mask.shape, device=x.device)
        drop = (rand < self.mask_ratio) & mask
        new_mask = mask & ~drop

        empty = new_mask.sum(dim=1) == 0
        if empty.any():
            # Restore each empty row's first originally-valid element.
            first_valid = mask.float().argmax(dim=1)
            new_mask[empty, first_valid[empty]] = True
        return new_mask
