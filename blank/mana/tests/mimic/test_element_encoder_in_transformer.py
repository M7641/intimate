"""Tests for SetTransformerEncoder with custom element_encoder."""

import torch
import torch.nn as nn

from mana.mimic import MLPElementEncoder, SetTransformerEncoder, TabularElementEncoder
from mana.mimic.data import SetBatch


class TestSetTransformerElementEncoder:
    def test_with_mlp_element_encoder(self, random_set_batch: SetBatch) -> None:
        enc = SetTransformerEncoder(
            input_dim=16,
            dim=32,
            output_dim=24,
            num_heads=4,
            element_encoder=MLPElementEncoder(16, 32, 32),
        )
        out = enc(random_set_batch.x, random_set_batch.mask)
        assert out.shape == (4, 24)

    def test_with_tabular_element_encoder(self) -> None:
        tab_enc = TabularElementEncoder(
            cat_cardinalities=[5, 3],
            num_continuous=2,
            embedding_dim=8,
            output_dim=32,
        )
        enc = SetTransformerEncoder(
            input_dim=4,  # 2 cats + 2 continuous
            dim=32,
            output_dim=16,
            num_heads=4,
            element_encoder=tab_enc,
        )
        # 2 categorical + 2 continuous features
        x = torch.zeros(2, 6, 4)
        x[:, :, 0] = torch.randint(0, 5, (2, 6)).float()  # cat 0
        x[:, :, 1] = torch.randint(0, 3, (2, 6)).float()  # cat 1
        x[:, :, 2:] = torch.randn(2, 6, 2)  # continuous
        mask = torch.ones(2, 6, dtype=torch.bool)

        out = enc(x, mask)
        assert out.shape == (2, 16)

    def test_default_still_uses_linear(self, random_set_batch: SetBatch) -> None:
        enc = SetTransformerEncoder(input_dim=16, dim=32, output_dim=24, num_heads=4)
        # proj_in should be nn.Linear when no element_encoder
        assert isinstance(enc.proj_in, torch.nn.Linear)
        out = enc(random_set_batch.x, random_set_batch.mask)
        assert out.shape == (4, 24)

    def test_element_encoder_receives_gradients(self) -> None:
        elem_enc = MLPElementEncoder(8, 32, 32)
        enc = SetTransformerEncoder(
            input_dim=8,
            dim=32,
            output_dim=16,
            num_heads=4,
            element_encoder=elem_enc,
        )
        x = torch.randn(2, 5, 8)
        mask = torch.ones(2, 5, dtype=torch.bool)
        out = enc(x, mask)
        out.sum().backward()
        has_grad = any(p.grad is not None for p in elem_enc.parameters())
        assert has_grad

    def test_permutation_invariance_with_element_encoder(self) -> None:
        elem_enc = MLPElementEncoder(8, 16, 16)
        enc = SetTransformerEncoder(
            input_dim=8,
            dim=16,
            output_dim=12,
            num_heads=2,
            element_encoder=elem_enc,
        )
        nn.Module.eval(enc)

        x = torch.randn(2, 6, 8)
        mask = torch.ones(2, 6, dtype=torch.bool)
        out1 = enc(x, mask)

        perm = torch.randperm(6)
        out2 = enc(x[:, perm], mask)
        assert torch.allclose(out1, out2, atol=1e-5)
