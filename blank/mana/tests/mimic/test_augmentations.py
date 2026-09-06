"""Tests for augmentations."""

import torch

from mana.mimic.augmentations import Compose, FeatureMask, FeatureNoise, SubsetSample


class TestFeatureNoise:
    def test_train_adds_noise(self) -> None:
        aug = FeatureNoise(std=0.1)
        aug.train()
        x = torch.randn(2, 5, 8)
        out, mask = aug(x)
        assert not torch.equal(x, out)
        assert mask is None

    def test_eval_passthrough(self) -> None:
        aug = FeatureNoise(std=0.1)
        aug.eval()
        x = torch.randn(2, 5, 8)
        out, mask = aug(x)
        assert torch.equal(x, out)
        assert mask is None

    def test_shape_preserved(self) -> None:
        aug = FeatureNoise(std=0.5)
        aug.train()
        x = torch.randn(3, 7, 16)
        out, _ = aug(x)
        assert out.shape == x.shape


class TestFeatureMask:
    def test_train_zeros_features(self) -> None:
        torch.manual_seed(42)
        aug = FeatureMask(p=0.5)
        aug.train()
        x = torch.ones(4, 6, 32)
        out, mask = aug(x)
        # Some features should be zeroed
        assert (out == 0).any()
        assert mask is None

    def test_eval_passthrough(self) -> None:
        aug = FeatureMask(p=0.5)
        aug.eval()
        x = torch.ones(2, 5, 8)
        out, mask = aug(x)
        assert torch.equal(x, out)

    def test_shape_preserved(self) -> None:
        aug = FeatureMask(p=0.3)
        aug.train()
        x = torch.randn(2, 5, 8)
        out, _ = aug(x)
        assert out.shape == x.shape


class TestSubsetSample:
    def test_train_drops_elements(self) -> None:
        torch.manual_seed(0)
        aug = SubsetSample(keep_fraction=0.5)
        aug.train()
        x = torch.randn(4, 20, 8)
        mask = torch.ones(4, 20, dtype=torch.bool)
        _, new_mask = aug(x, mask)
        assert new_mask is not None
        # Some elements should be dropped
        assert new_mask.sum() < mask.sum()

    def test_at_least_one_kept(self) -> None:
        torch.manual_seed(0)
        aug = SubsetSample(keep_fraction=0.01)  # Very aggressive dropout
        aug.train()
        x = torch.randn(8, 10, 4)
        mask = torch.ones(8, 10, dtype=torch.bool)
        _, new_mask = aug(x, mask)
        assert new_mask is not None
        # Every sample must have at least 1 valid element
        assert (new_mask.sum(dim=1) >= 1).all()

    def test_eval_passthrough(self) -> None:
        aug = SubsetSample(keep_fraction=0.5)
        aug.eval()
        x = torch.randn(2, 5, 8)
        mask = torch.ones(2, 5, dtype=torch.bool)
        out, new_mask = aug(x, mask)
        assert torch.equal(x, out)
        assert torch.equal(mask, new_mask)

    def test_respects_existing_mask(self) -> None:
        torch.manual_seed(42)
        aug = SubsetSample(keep_fraction=0.5)
        aug.train()
        x = torch.randn(2, 10, 4)
        mask = torch.zeros(2, 10, dtype=torch.bool)
        mask[:, :3] = True  # Only first 3 are valid
        _, new_mask = aug(x, mask)
        assert new_mask is not None
        # Should only keep elements from the originally valid ones
        assert not new_mask[:, 3:].any()


class TestCompose:
    def test_chains_augmentations(self) -> None:
        aug = Compose([FeatureNoise(std=0.1), FeatureMask(p=0.2)])
        aug.train()
        x = torch.ones(2, 5, 8)
        out, mask = aug(x)
        assert out.shape == x.shape
        # Should have both noise and zeroed features
        assert not torch.equal(x, out)

    def test_threads_mask(self) -> None:
        aug = Compose([SubsetSample(keep_fraction=0.7), FeatureNoise(std=0.1)])
        aug.train()
        x = torch.randn(2, 10, 8)
        mask = torch.ones(2, 10, dtype=torch.bool)
        _, new_mask = aug(x, mask)
        assert new_mask is not None
