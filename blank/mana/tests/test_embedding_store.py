"""Tests for EmbeddingStore additions: __getitem__, split, from_mimic with SetDataset."""

import torch
import torch.nn as nn
import pytest

from mana.embeddings import EmbeddingStore
from mana.mimic import (
    Compose,
    ContrastiveTrainer,
    DeepSetsEncoder,
    FeatureNoise,
    MeanAggregator,
    MLPElementEncoder,
    MLPProjector,
    MimicModel,
    NTXentLoss,
    SetDataset,
    TrainConfig,
    set_dataloader,
)


def _make_store(n: int = 20, dim: int = 8) -> EmbeddingStore:
    ids = [f"e{i}" for i in range(n)]
    embeddings = torch.randn(n, dim)
    return EmbeddingStore(ids=ids, embeddings=embeddings)


def _make_model_and_dataset():
    torch.manual_seed(0)
    sets = [torch.randn(torch.randint(3, 8, (1,)).item(), 4) for _ in range(16)]
    dataset = SetDataset(sets, ids=[f"id_{i}" for i in range(16)])
    model = MimicModel(
        encoder=DeepSetsEncoder(
            element_encoder=MLPElementEncoder(4, 16, 16),
            aggregator=MeanAggregator(),
            rho=nn.Sequential(nn.Linear(16, 16), nn.ReLU()),
            output_dim=16,
        ),
        projector=MLPProjector(16, 8, 8),
        augmentation=Compose([FeatureNoise(std=0.1)]),
        loss_fn=NTXentLoss(temperature=0.1),
    )
    # Quick train so encode_all works
    dl = set_dataloader(dataset, batch_size=8, shuffle=True)
    ContrastiveTrainer(model, TrainConfig(epochs=2, verbose=False)).fit(dl)
    return model, dataset


class TestGetItem:
    def test_getitem_single_id(self) -> None:
        store = _make_store()
        emb = store["e3"]
        assert emb.shape == (8,)
        assert torch.equal(emb, store.embeddings[3])

    def test_getitem_list_of_ids(self) -> None:
        store = _make_store()
        embs = store[["e1", "e5", "e0"]]
        assert embs.shape == (3, 8)
        assert torch.equal(embs[0], store.embeddings[1])
        assert torch.equal(embs[2], store.embeddings[0])

    def test_getitem_unknown_id_raises(self) -> None:
        store = _make_store()
        with pytest.raises(ValueError):
            store["nonexistent"]


class TestSplit:
    def test_split_sizes(self) -> None:
        store = _make_store(n=100)
        train, test = store.split(test_ratio=0.2, seed=42)
        assert len(train) + len(test) == 100
        assert len(test) == 20

    def test_split_deterministic(self) -> None:
        store = _make_store(n=50)
        t1, v1 = store.split(test_ratio=0.3, seed=99)
        t2, v2 = store.split(test_ratio=0.3, seed=99)
        assert t1.ids == t2.ids
        assert v1.ids == v2.ids

    def test_split_no_id_overlap(self) -> None:
        store = _make_store(n=50)
        train, test = store.split(test_ratio=0.2, seed=0)
        assert set(train.ids).isdisjoint(set(test.ids))


class TestFromMimicWithDataset:
    def test_from_mimic_with_dataset(self) -> None:
        model, dataset = _make_model_and_dataset()
        store = EmbeddingStore.from_mimic(model, dataset)
        assert len(store) == len(dataset)
        assert store.dim > 0

    def test_from_mimic_with_dataset_uses_ids(self) -> None:
        model, dataset = _make_model_and_dataset()
        store = EmbeddingStore.from_mimic(model, dataset)
        assert store.ids == dataset.ids

    def test_from_mimic_with_dataloader_backwards_compat(self) -> None:
        model, dataset = _make_model_and_dataset()
        dl = set_dataloader(dataset, batch_size=16, shuffle=False)
        ids = [f"id_{i}" for i in range(16)]
        store = EmbeddingStore.from_mimic(model, dl, ids=ids)
        assert store.ids == ids
        assert len(store) == 16

    def test_from_mimic_dataset_no_ids_raises(self) -> None:
        model, _ = _make_model_and_dataset()
        dataset_no_ids = SetDataset([torch.randn(3, 4) for _ in range(8)])
        with pytest.raises(ValueError, match="ids must be provided"):
            EmbeddingStore.from_mimic(model, dataset_no_ids)


class TestSaveLoadRoundtrip:
    def test_save_load_roundtrip(self, tmp_path) -> None:
        store = _make_store(n=10, dim=4)
        path = tmp_path / "store.pt"
        store.save(path)
        loaded = EmbeddingStore.load(path)
        assert loaded.ids == store.ids
        assert torch.equal(loaded.embeddings, store.embeddings)
        # Verify new features work on loaded store
        assert torch.equal(loaded["e3"], store["e3"])
        t1, t2 = loaded.split(test_ratio=0.3, seed=1)
        assert len(t1) + len(t2) == 10
