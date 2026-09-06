from __future__ import annotations

import torch
import torch.nn as nn
from torch import Tensor


class TabularElementEncoder(nn.Module):
    """Encodes tabular set elements with mixed feature types.

    Each set element is a row with categorical and continuous columns.
    Categoricals get embedding lookups, continuous features get a linear layer,
    both are concatenated and passed through an MLP.

    Input tensor layout per element: [cat_0, cat_1, ..., cat_k, cont_0, cont_1, ..., cont_m]
    Categorical values should be integer indices (will be cast to long internally).

    Input:  (B, N, num_cats + num_continuous)
    Output: (B, N, output_dim)
    """

    def __init__(
        self,
        cat_cardinalities: list[int],
        num_continuous: int,
        embedding_dim: int,
        output_dim: int,
        hidden_dim: int | None = None,
        dropout: float = 0.0,
    ) -> None:
        super().__init__()
        self._num_cats = len(cat_cardinalities)
        self._num_continuous = num_continuous

        self.embeddings = nn.ModuleList(
            [nn.Embedding(card, embedding_dim) for card in cat_cardinalities]
        )
        cat_total = len(cat_cardinalities) * embedding_dim
        cont_out = num_continuous  # pass-through if no continuous
        if num_continuous > 0:
            self.cont_linear = nn.Linear(num_continuous, num_continuous)
        else:
            self.cont_linear = None

        concat_dim = cat_total + cont_out
        h = hidden_dim or concat_dim
        self.mlp = nn.Sequential(
            nn.Linear(concat_dim, h),
            nn.ReLU(),
            nn.Dropout(dropout),
            nn.Linear(h, output_dim),
        )

    def forward(self, x: Tensor) -> Tensor:
        parts: list[Tensor] = []
        # Categorical embeddings
        for i, emb in enumerate(self.embeddings):
            indices = x[..., i].long()
            parts.append(emb(indices))
        # Continuous features
        if self._num_continuous > 0:
            cont = x[..., self._num_cats :]
            assert self.cont_linear is not None
            parts.append(self.cont_linear(cont))
        out = torch.cat(parts, dim=-1)
        return self.mlp(out)


class MLPElementEncoder(nn.Module):
    """Encodes individual set elements through an MLP.

    Input:  (B, N, input_dim)
    Output: (B, N, output_dim)
    """

    def __init__(
        self,
        input_dim: int,
        hidden_dim: int,
        output_dim: int,
        num_layers: int = 2,
        activation: type[nn.Module] = nn.ReLU,
        dropout: float = 0.0,
    ) -> None:
        super().__init__()
        layers: list[nn.Module] = []
        in_dim = input_dim
        for _ in range(num_layers - 1):
            layers.extend(
                [nn.Linear(in_dim, hidden_dim), activation(), nn.Dropout(dropout)]
            )
            in_dim = hidden_dim
        layers.append(nn.Linear(in_dim, output_dim))
        self.net = nn.Sequential(*layers)

    def forward(self, x: Tensor) -> Tensor:
        return self.net(x)
