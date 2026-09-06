"""Tests for DeepSetsEncoder."""

import torch
import torch.nn as nn

from mana.mimic.aggregators import MeanAggregator
from mana.mimic.data import SetBatch
from mana.mimic.element_encoders import MLPElementEncoder
from mana.mimic.encoders import DeepSetsEncoder


def _make_deep_sets(
    input_dim: int = 16, hidden: int = 32, output: int = 24
) -> DeepSetsEncoder:
    return DeepSetsEncoder(
        element_encoder=MLPElementEncoder(input_dim, hidden, hidden),
        aggregator=MeanAggregator(),
        rho=nn.Sequential(nn.Linear(hidden, output), nn.ReLU()),
        output_dim=output,
    )


class TestDeepSetsEncoder:
    def test_output_shape(self, random_set_batch: SetBatch) -> None:
        enc = _make_deep_sets()
        out = enc(random_set_batch.x, random_set_batch.mask)
        assert out.shape == (4, 24)

    def test_permutation_invariance(self) -> None:
        """Randomly permuting set elements should not change the output."""
        enc = _make_deep_sets(input_dim=8)
        enc.eval()

        x = torch.randn(2, 6, 8)
        mask = torch.ones(2, 6, dtype=torch.bool)

        out1 = enc(x, mask)

        # Permute elements within each set
        perm = torch.randperm(6)
        x_perm = x[:, perm]
        out2 = enc(x_perm, mask)

        assert torch.allclose(out1, out2, atol=1e-5)

    def test_output_dim_property(self) -> None:
        enc = _make_deep_sets(output=64)
        assert enc.output_dim == 64
