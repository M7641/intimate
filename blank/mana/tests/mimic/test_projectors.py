"""Tests for projectors."""

import torch

from mana.mimic.projectors import MLPProjector


class TestMLPProjector:
    def test_output_shape(self) -> None:
        proj = MLPProjector(input_dim=64, hidden_dim=32, output_dim=16)
        x = torch.randn(4, 64)
        out = proj(x)
        assert out.shape == (4, 16)

    def test_normalized(self) -> None:
        proj = MLPProjector(input_dim=64, hidden_dim=32, output_dim=16, normalize=True)
        x = torch.randn(4, 64)
        out = proj(x)
        norms = out.norm(dim=-1)
        assert torch.allclose(norms, torch.ones(4), atol=1e-5)

    def test_unnormalized(self) -> None:
        proj = MLPProjector(input_dim=64, hidden_dim=32, output_dim=16, normalize=False)
        x = torch.randn(4, 64)
        out = proj(x)
        norms = out.norm(dim=-1)
        # Unnormalized output should generally NOT be unit norm
        assert not torch.allclose(norms, torch.ones(4), atol=1e-2)
