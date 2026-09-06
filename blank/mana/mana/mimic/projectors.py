from __future__ import annotations

import torch.nn as nn
import torch.nn.functional as F
from torch import Tensor


def make_decoder(input_dim: int, hidden_dim: int, output_dim: int) -> nn.Sequential:
    """Simple MLP decoder for reconstruction loss (no BatchNorm, no L2 norm)."""
    return nn.Sequential(
        nn.Linear(input_dim, hidden_dim),
        nn.ReLU(),
        nn.Linear(hidden_dim, output_dim),
    )


class MLPProjector(nn.Module):
    """Linear → BatchNorm → ReLU → Linear, with optional L2 normalization.

    Input:  (B, input_dim)
    Output: (B, output_dim)
    """

    def __init__(
        self,
        input_dim: int,
        hidden_dim: int,
        output_dim: int,
        normalize: bool = True,
    ) -> None:
        super().__init__()
        self.normalize = normalize
        self.net = nn.Sequential(
            nn.Linear(input_dim, hidden_dim),
            nn.BatchNorm1d(hidden_dim),
            nn.ReLU(),
            nn.Linear(hidden_dim, output_dim),
        )

    def forward(self, x: Tensor) -> Tensor:
        z = self.net(x)
        if self.normalize:
            z = F.normalize(z, dim=-1)
        return z
