"""Tests for element encoders."""

import torch

from mana.mimic.element_encoders import MLPElementEncoder, TabularElementEncoder


class TestMLPElementEncoder:
    def test_output_shape(self) -> None:
        enc = MLPElementEncoder(input_dim=16, hidden_dim=32, output_dim=24)
        x = torch.randn(4, 10, 16)
        out = enc(x)
        assert out.shape == (4, 10, 24)

    def test_custom_layers(self) -> None:
        enc = MLPElementEncoder(
            input_dim=8, hidden_dim=16, output_dim=12, num_layers=3, dropout=0.1
        )
        x = torch.randn(2, 5, 8)
        out = enc(x)
        assert out.shape == (2, 5, 12)

    def test_single_layer(self) -> None:
        enc = MLPElementEncoder(input_dim=8, hidden_dim=16, output_dim=12, num_layers=1)
        x = torch.randn(2, 5, 8)
        out = enc(x)
        assert out.shape == (2, 5, 12)


class TestTabularElementEncoder:
    def test_output_shape(self) -> None:
        enc = TabularElementEncoder(
            cat_cardinalities=[5, 10, 3],
            num_continuous=4,
            embedding_dim=8,
            output_dim=32,
        )
        # 3 cat columns + 4 continuous columns = 7 features per element
        x = torch.zeros(2, 6, 7)
        x[..., :3] = torch.randint(0, 3, (2, 6, 3)).float()
        x[..., 3:] = torch.randn(2, 6, 4)
        out = enc(x)
        assert out.shape == (2, 6, 32)

    def test_cats_only(self) -> None:
        enc = TabularElementEncoder(
            cat_cardinalities=[5, 10],
            num_continuous=0,
            embedding_dim=4,
            output_dim=16,
        )
        x = torch.randint(0, 5, (3, 4, 2)).float()
        out = enc(x)
        assert out.shape == (3, 4, 16)
