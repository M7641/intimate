"""Shared fixtures for mimic tests."""

import pytest
import torch

from mana.mimic.data import SetBatch


@pytest.fixture
def random_set_batch() -> SetBatch:
    """A batch of 4 sets with max 10 elements, dim 16, variable lengths."""
    B, N_max, D = 4, 10, 16
    x = torch.randn(B, N_max, D)
    mask = torch.ones(B, N_max, dtype=torch.bool)
    # Make sets variable-length: lengths [10, 7, 5, 3]
    lengths = [10, 7, 5, 3]
    for i, length in enumerate(lengths):
        mask[i, length:] = False
        x[i, length:] = 0.0
    return SetBatch(x=x, mask=mask)
