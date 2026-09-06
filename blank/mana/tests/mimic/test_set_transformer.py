"""Tests for Set Transformer components."""

import torch

from mana.mimic.data import SetBatch
from mana.mimic.encoders import MAB, PMA, SAB, SetTransformerEncoder
from mana.mimic.encoders import ISAB


class TestMAB:
    def test_output_shape(self) -> None:
        mab = MAB(dim=32, num_heads=4)
        x = torch.randn(2, 5, 32)
        y = torch.randn(2, 8, 32)
        out = mab(x, y)
        assert out.shape == (2, 5, 32)

    def test_with_key_padding_mask(self) -> None:
        mab = MAB(dim=32, num_heads=4)
        x = torch.randn(2, 5, 32)
        y = torch.randn(2, 8, 32)
        kpm = torch.zeros(2, 8, dtype=torch.bool)
        kpm[0, 6:] = True  # mask last 2 in first batch
        out = mab(x, y, key_padding_mask=kpm)
        assert out.shape == (2, 5, 32)


class TestSAB:
    def test_output_shape(self) -> None:
        sab = SAB(dim=32, num_heads=4)
        x = torch.randn(2, 10, 32)
        out = sab(x)
        assert out.shape == (2, 10, 32)


class TestISAB:
    def test_output_shape(self) -> None:
        isab = ISAB(dim=32, num_heads=4, num_inducing_points=5)
        x = torch.randn(2, 10, 32)
        out = isab(x)
        assert out.shape == (2, 10, 32)


class TestPMA:
    def test_output_shape(self) -> None:
        pma = PMA(dim=32, num_heads=4, num_seeds=1)
        x = torch.randn(2, 10, 32)
        out = pma(x)
        assert out.shape == (2, 1, 32)

    def test_multi_seed(self) -> None:
        pma = PMA(dim=32, num_heads=4, num_seeds=3)
        x = torch.randn(2, 10, 32)
        out = pma(x)
        assert out.shape == (2, 3, 32)


class TestSABMaskInvariance:
    def test_mask_invariance(self) -> None:
        """Padded garbage values should not affect output when properly masked."""
        sab = SAB(dim=16, num_heads=2)
        sab.eval()

        B, N_real, N_pad = 2, 5, 8
        x_clean = torch.randn(B, N_real, 16)

        # No padding — all valid
        out_clean = sab(x_clean)

        # Pad with large garbage, mask it out
        garbage = torch.randn(B, N_pad - N_real, 16) * 1000.0
        x_padded = torch.cat([x_clean, garbage], dim=1)
        kpm = torch.zeros(B, N_pad, dtype=torch.bool)
        kpm[:, N_real:] = True  # mask garbage positions

        out_padded = sab(x_padded, key_padding_mask=kpm)[:, :N_real]

        assert torch.allclose(out_clean, out_padded, atol=1e-5)


class TestISABMaskInvariance:
    def test_mask_invariance_isab(self) -> None:
        """Padded garbage values should not affect ISAB output when masked."""
        isab = ISAB(dim=16, num_heads=2, num_inducing_points=4)
        isab.eval()

        B, N_real, N_pad = 2, 5, 8
        x_clean = torch.randn(B, N_real, 16)

        out_clean = isab(x_clean)

        garbage = torch.randn(B, N_pad - N_real, 16) * 1000.0
        x_padded = torch.cat([x_clean, garbage], dim=1)
        kpm = torch.zeros(B, N_pad, dtype=torch.bool)
        kpm[:, N_real:] = True

        out_padded = isab(x_padded, key_padding_mask=kpm)[:, :N_real]

        assert torch.allclose(out_clean, out_padded, atol=1e-5)


class TestGradientFlow:
    # NOTE: these use a quadratic objective, not out.sum(). The block ends in a
    # LayerNorm (gamma=1, beta=0 at init), and the sum over its normalised
    # dimension cancels to exactly 0 — including in the backward pass — leaving
    # only floating-point dust whose sign depends on RNG / test ordering.
    # out.pow(2).sum() breaks that linear cancellation and tests real gradient
    # flow to the learnable inducing points / seeds.

    def test_isab_inducing_points_receive_gradients(self) -> None:
        isab = ISAB(dim=16, num_heads=2, num_inducing_points=4)
        x = torch.randn(2, 6, 16)
        out = isab(x)
        out.pow(2).sum().backward()
        assert isab.inducing_points.grad is not None
        assert isab.inducing_points.grad.abs().sum() > 0

    def test_pma_seeds_receive_gradients(self) -> None:
        pma = PMA(dim=16, num_heads=2, num_seeds=3)
        x = torch.randn(2, 6, 16)
        out = pma(x)
        out.pow(2).sum().backward()
        assert pma.seeds.grad is not None
        assert pma.seeds.grad.abs().sum() > 0


class TestSetTransformerEncoder:
    def test_output_shape_sab(self, random_set_batch: SetBatch) -> None:
        enc = SetTransformerEncoder(input_dim=16, dim=32, output_dim=24, num_heads=4)
        out = enc(random_set_batch.x, random_set_batch.mask)
        assert out.shape == (4, 24)

    def test_output_shape_isab(self, random_set_batch: SetBatch) -> None:
        enc = SetTransformerEncoder(
            input_dim=16, dim=32, output_dim=24, num_heads=4, num_inducing_points=4
        )
        out = enc(random_set_batch.x, random_set_batch.mask)
        assert out.shape == (4, 24)

    def test_permutation_invariance(self) -> None:
        enc = SetTransformerEncoder(input_dim=8, dim=16, output_dim=12, num_heads=2)
        enc.eval()

        x = torch.randn(2, 6, 8)
        mask = torch.ones(2, 6, dtype=torch.bool)

        out1 = enc(x, mask)

        perm = torch.randperm(6)
        x_perm = x[:, perm]
        out2 = enc(x_perm, mask)

        assert torch.allclose(out1, out2, atol=1e-5)

    def test_output_dim_property(self) -> None:
        enc = SetTransformerEncoder(input_dim=8, dim=16, output_dim=32, num_heads=2)
        assert enc.output_dim == 32
