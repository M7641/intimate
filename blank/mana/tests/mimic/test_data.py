"""Tests for data utilities."""

import torch

from mana.mimic.data import SetBatch, SetDataset, collate_sets, set_dataloader


class TestCollate:
    def test_pads_to_max_length(self) -> None:
        sets = [torch.randn(3, 8), torch.randn(7, 8), torch.randn(5, 8)]
        batch = collate_sets(sets)
        assert batch.x.shape == (3, 7, 8)
        assert batch.mask.shape == (3, 7)

    def test_mask_correctness(self) -> None:
        sets = [torch.randn(2, 4), torch.randn(5, 4)]
        batch = collate_sets(sets)
        assert batch.mask[0].tolist() == [True, True, False, False, False]
        assert batch.mask[1].tolist() == [True, True, True, True, True]

    def test_padded_values_zero(self) -> None:
        sets = [torch.ones(2, 3), torch.ones(4, 3)]
        batch = collate_sets(sets)
        assert (batch.x[0, 2:] == 0).all()

    def test_single_set(self) -> None:
        sets = [torch.randn(5, 4)]
        batch = collate_sets(sets)
        assert batch.x.shape == (1, 5, 4)
        assert batch.mask.all()


class TestSetDataset:
    def test_len(self) -> None:
        ds = SetDataset([torch.randn(3, 4), torch.randn(5, 4)])
        assert len(ds) == 2

    def test_getitem(self) -> None:
        t = torch.randn(3, 4)
        ds = SetDataset([t])
        assert torch.equal(ds[0], t)

    def test_setdataset_with_ids(self) -> None:
        sets = [torch.randn(3, 4), torch.randn(5, 4)]
        ds = SetDataset(sets, ids=["a", "b"])
        assert ds.ids == ["a", "b"]

    def test_setdataset_ids_length_mismatch_raises(self) -> None:
        import pytest

        sets = [torch.randn(3, 4), torch.randn(5, 4)]
        with pytest.raises(ValueError, match="ids length"):
            SetDataset(sets, ids=["a"])

    def test_setdataset_getitem_returns_tuple_with_ids(self) -> None:
        t = torch.randn(3, 4)
        ds = SetDataset([t], ids=["x"])
        result = ds[0]
        assert isinstance(result, tuple)
        assert torch.equal(result[0], t)
        assert result[1] == "x"

    def test_setdataset_getitem_returns_tensor_without_ids(self) -> None:
        t = torch.randn(3, 4)
        ds = SetDataset([t])
        result = ds[0]
        assert isinstance(result, torch.Tensor)
        assert torch.equal(result, t)


class TestCollateIds:
    def test_collate_propagates_ids(self) -> None:
        batch = [(torch.randn(3, 4), "a"), (torch.randn(5, 4), "b")]
        result = collate_sets(batch)
        assert result.ids == ["a", "b"]
        assert result.x.shape == (2, 5, 4)

    def test_collate_without_ids_still_works(self) -> None:
        batch = [torch.randn(3, 4), torch.randn(5, 4)]
        result = collate_sets(batch)
        assert result.ids is None
        assert result.x.shape == (2, 5, 4)


class TestSetDataloader:
    def test_iteration(self) -> None:
        sets = [torch.randn(i + 2, 8) for i in range(10)]
        ds = SetDataset(sets)
        dl = set_dataloader(ds, batch_size=4, shuffle=False)
        batches = list(dl)
        assert len(batches) == 3  # 10 / 4 = 2.5 → 3 batches
        assert isinstance(batches[0], SetBatch)
        assert batches[0].x.shape[0] == 4
