"""Simple permutation-invariant aggregator: mean."""

from __future__ import annotations

import torch.nn as nn
from torch import Tensor


class MeanAggregator(nn.Module):
    """Mean over valid set elements, respecting mask. (B, N, D) → (B, D)."""

    def forward(self, x: Tensor, mask: Tensor | None = None) -> Tensor:
        if mask is not None:
            x = x * mask.unsqueeze(-1)
            counts = mask.sum(dim=1, keepdim=True).clamp(min=1)
            return x.sum(dim=1) / counts
        return x.mean(dim=1)
