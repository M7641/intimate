"""Tests for curriculum augmentation."""

import torch

from mana.mimic.augmentations import CurriculumAugmentation, FeatureNoise, FeatureMask


class TestCurriculumAugmentation:
    def test_ramps_noise_std(self) -> None:
        base = FeatureNoise(std=0.5)
        cur = CurriculumAugmentation(base, start_strength=0.1, end_strength=1.0)
        cur.train()

        # At start (progress=0), std should be scaled down
        cur.set_progress(0.0)
        assert abs(base.std - 0.5 * 0.1) < 1e-6

        # At end (progress=1), std should be at full
        cur.set_progress(1.0)
        assert abs(base.std - 0.5 * 1.0) < 1e-6

    def test_ramps_mask_p(self) -> None:
        base = FeatureMask(p=0.3)
        cur = CurriculumAugmentation(base, start_strength=0.0, end_strength=1.0)

        cur.set_progress(0.0)
        assert abs(base.p - 0.0) < 1e-6

        cur.set_progress(1.0)
        assert abs(base.p - 0.3) < 1e-6

    def test_forward_works(self) -> None:
        base = FeatureNoise(std=0.1)
        cur = CurriculumAugmentation(base)
        cur.train()
        cur.set_progress(0.5)

        x = torch.randn(2, 5, 8)
        out, mask = cur(x)
        assert out.shape == x.shape

    def test_progress_clamped(self) -> None:
        base = FeatureNoise(std=0.1)
        cur = CurriculumAugmentation(base)
        cur.set_progress(-1.0)  # Should clamp to 0
        assert cur._progress == 0.0
        cur.set_progress(2.0)  # Should clamp to 1
        assert cur._progress == 1.0
