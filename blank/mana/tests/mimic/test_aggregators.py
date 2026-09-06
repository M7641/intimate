"""Tests for aggregators."""

import torch

from mana.mimic.aggregators import MeanAggregator
from mana.mimic.data import SetBatch


class TestMeanAggregator:
    def test_output_shape(self, random_set_batch: SetBatch) -> None:
        agg = MeanAggregator()
        out = agg(random_set_batch.x, random_set_batch.mask)
        assert out.shape == (4, 16)

    def test_mask_correctness(self) -> None:
        agg = MeanAggregator()
        x = torch.tensor([[[1.0, 2.0], [3.0, 4.0], [99.0, 99.0]]])
        mask = torch.tensor([[True, True, False]])
        out = agg(x, mask)
        assert torch.allclose(out, torch.tensor([[2.0, 3.0]]))

    def test_padded_elements_ignored(self) -> None:
        """Padded elements should not affect the mean."""
        agg = MeanAggregator()
        x = torch.tensor([[[1.0], [1.0], [1000.0]]])
        mask = torch.tensor([[True, True, False]])
        out = agg(x, mask)
        assert torch.allclose(out, torch.tensor([[1.0]]))
