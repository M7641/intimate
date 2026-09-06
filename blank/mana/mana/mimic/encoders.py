from __future__ import annotations

import torch
import torch.nn as nn
from torch import Tensor

from .types import Aggregator, ElementEncoder


class DeepSetsEncoder(nn.Module):
    """Deep Sets: encode each element, aggregate, then transform.

    Architecture: x → phi(x) → aggregate → rho → output

    Input:  (B, N, input_dim)
    Output: (B, output_dim)
    """

    def __init__(
        self,
        element_encoder: ElementEncoder,
        aggregator: Aggregator,
        rho: nn.Module,
        output_dim: int,
    ) -> None:
        super().__init__()
        self.element_encoder = element_encoder
        self.aggregator = aggregator
        self.rho = rho
        self._output_dim = output_dim

    @property
    def output_dim(self) -> int:
        return self._output_dim

    def forward(self, x: Tensor, mask: Tensor | None = None) -> Tensor:
        h = self.element_encoder(x)  # (B, N, D)
        h = self.aggregator(h, mask)  # (B, D)
        return self.rho(h)  # (B, output_dim)


class MAB(nn.Module):
    """Multihead Attention Block: MAB(X, Y) = LayerNorm(H + rFF(H))
    where H = LayerNorm(X + MultiheadAttention(X, Y, Y)).
    """

    def __init__(self, dim: int, num_heads: int, dropout: float = 0.0) -> None:
        super().__init__()
        self.attn = nn.MultiheadAttention(
            dim, num_heads, dropout=dropout, batch_first=True
        )
        self.norm1 = nn.LayerNorm(dim)
        self.norm2 = nn.LayerNorm(dim)
        self.ff = nn.Sequential(
            nn.Linear(dim, dim), nn.ReLU(), nn.Dropout(dropout), nn.Linear(dim, dim)
        )

    def forward(
        self, x: Tensor, y: Tensor, key_padding_mask: Tensor | None = None
    ) -> Tensor:
        h, _ = self.attn(x, y, y, key_padding_mask=key_padding_mask)
        h = self.norm1(x + h)
        return self.norm2(h + self.ff(h))


class SAB(nn.Module):
    """Set Attention Block: SAB(X) = MAB(X, X). Full O(n^2) self-attention."""

    def __init__(self, dim: int, num_heads: int, dropout: float = 0.0) -> None:
        super().__init__()
        self.mab = MAB(dim, num_heads, dropout)

    def forward(self, x: Tensor, key_padding_mask: Tensor | None = None) -> Tensor:
        return self.mab(x, x, key_padding_mask)


class ISAB(nn.Module):
    """Induced Set Attention Block: O(nm) attention via inducing points.

    Uses m learnable inducing points to reduce the O(n^2) self-attention
    to two O(nm) attention operations.
    """

    def __init__(
        self, dim: int, num_heads: int, num_inducing_points: int, dropout: float = 0.0
    ) -> None:
        super().__init__()
        self.inducing_points = nn.Parameter(
            nn.init.xavier_uniform_(torch.empty(1, num_inducing_points, dim))
        )
        self.mab1 = MAB(dim, num_heads, dropout)
        self.mab2 = MAB(dim, num_heads, dropout)

    def forward(self, x: Tensor, key_padding_mask: Tensor | None = None) -> Tensor:
        batch_size = x.shape[0]
        inducing = self.inducing_points.expand(batch_size, -1, -1)
        h = self.mab1(inducing, x, key_padding_mask)  # (B, m, D)
        return self.mab2(x, h)  # (B, N, D) — no mask for inducing points


class PMA(nn.Module):
    """Pooling by Multihead Attention: aggregates a set into k output vectors.

    Uses k learnable seed vectors to attend over the set.
    """

    def __init__(
        self, dim: int, num_heads: int, num_seeds: int = 1, dropout: float = 0.0
    ) -> None:
        super().__init__()
        self.seeds = nn.Parameter(
            nn.init.xavier_uniform_(torch.empty(1, num_seeds, dim))
        )
        self.mab = MAB(dim, num_heads, dropout)

    def forward(self, x: Tensor, key_padding_mask: Tensor | None = None) -> Tensor:
        batch_size = x.shape[0]
        S = self.seeds.expand(batch_size, -1, -1)
        return self.mab(S, x, key_padding_mask)  # (B, k, D)


class SetTransformerEncoder(nn.Module):
    """Full Set Transformer encoder.

    Architecture: Linear projection → SAB/ISAB stack → PMA → Linear output

    When num_inducing_points is None, uses SAB (full O(n^2) attention).
    When set, uses ISAB with that many inducing points (O(nm) attention).

    Input:  (B, N, input_dim)
    Output: (B, output_dim)
    """

    def __init__(
        self,
        input_dim: int,
        # TUNING: dim is the internal hidden dimension of all attention layers.
        # Controls model capacity. 32-64 for small datasets (<1k samples),
        # 128-256 for larger ones. Must be divisible by num_heads.
        dim: int,
        # TUNING: output_dim is the embedding size used downstream. Smaller
        # (16-32) forces compression and can improve generalisation. Larger
        # (64-256) preserves more information but may overfit. Match to the
        # complexity of your downstream task.
        output_dim: int,
        num_heads: int = 4,
        # TUNING: num_layers controls attention depth. 1-2 is sufficient for
        # most set sizes. 3-4 may help with complex inter-element relationships
        # but risks overfitting on small datasets. Diminishing returns beyond 4.
        num_layers: int = 2,
        num_inducing_points: int | None = None,
        dropout: float = 0.0,
        element_encoder: ElementEncoder | None = None,
    ) -> None:
        super().__init__()
        self._output_dim = output_dim

        if element_encoder is not None:
            self.proj_in = element_encoder
        else:
            self.proj_in = nn.Linear(input_dim, dim)

        layers: list[nn.Module] = []
        for _ in range(num_layers):
            if num_inducing_points is not None:
                layers.append(ISAB(dim, num_heads, num_inducing_points, dropout))
            else:
                layers.append(SAB(dim, num_heads, dropout))
        self.encoder_stack = nn.ModuleList(layers)

        self.pma = PMA(dim, num_heads, num_seeds=1, dropout=dropout)
        self.proj_out = nn.Linear(dim, output_dim)

    @property
    def output_dim(self) -> int:
        return self._output_dim

    def forward(self, x: Tensor, mask: Tensor | None = None) -> Tensor:
        # Invert mask for PyTorch's key_padding_mask (True = ignored)
        kpm = ~mask if mask is not None else None

        h = self.proj_in(x)  # (B, N, dim)
        for layer in self.encoder_stack:
            h = layer(h, key_padding_mask=kpm)
        h = self.pma(h, key_padding_mask=kpm)  # (B, 1, dim)
        return self.proj_out(h.squeeze(1))  # (B, output_dim)
