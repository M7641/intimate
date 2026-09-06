"""Exponential Moving Average encoder for momentum-based contrastive learning."""

from __future__ import annotations

import copy
import math

import torch
import torch.nn as nn
from torch import Tensor


class EMAEncoder(nn.Module):
    """Maintains an EMA copy of an encoder's weights.

    Shadow parameters are updated as:
        theta_ema = momentum * theta_ema + (1 - momentum) * theta_main

    When cosine_schedule=True, momentum ramps from momentum_base toward 1.0
    over training via: m(t) = 1 - (1 - m_base)(1 + cos(pi * t)) / 2.

    Shadow parameters have requires_grad=False.
    """

    def __init__(
        self,
        encoder: nn.Module,
        # TUNING: momentum controls how slowly the shadow tracks the main encoder.
        # Higher (0.999) = very stable targets, good for large datasets with many
        # steps per epoch. Lower (0.99) = faster tracking, better for small datasets
        # where the encoder changes significantly each epoch. With cosine_schedule=True,
        # this is the starting value — it ramps toward 1.0 during training.
        momentum: float = 0.996,
        cosine_schedule: bool = True,
    ) -> None:
        super().__init__()
        self.momentum_base = momentum
        self.cosine_schedule = cosine_schedule
        self._current_momentum = momentum

        self.shadow = copy.deepcopy(encoder)
        for p in self.shadow.parameters():
            p.requires_grad_(False)

    @torch.no_grad()
    def update(self, encoder: nn.Module) -> None:
        """Update shadow weights from the main encoder."""
        m = self._current_momentum
        for shadow_p, main_p in zip(
            self.shadow.parameters(), encoder.parameters(), strict=True
        ):
            shadow_p.data.mul_(m).add_(main_p.data, alpha=1.0 - m)

    def set_progress(self, fraction: float) -> None:
        """Update momentum via cosine schedule. fraction in [0, 1]."""
        if self.cosine_schedule:
            self._current_momentum = (
                1.0
                - (1.0 - self.momentum_base)
                * (1.0 + math.cos(math.pi * fraction))
                / 2.0
            )

    def forward(self, x: Tensor, mask: Tensor | None = None) -> Tensor:
        return self.shadow(x, mask)
