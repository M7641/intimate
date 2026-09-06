# Mana Architecture

## The Pipeline

Mana is organised as a linear pipeline. Each stage produces an artifact the next stage consumes, and every stage is independently useful.

```
Raw Data
  |
  v
ENCODERS ─── transform categorical / numeric / text into tensors
  |
  v
MIMIC ────── contrastive learning on sets → fixed-size embeddings
  |
  v
EMBEDDING STORE ── serialisable (N, D) tensor + IDs + metadata
  |
  v
ADAPTERS ─── consume embeddings for a specific task
  |           (recommend, forecast, classify, regress)
  v
SERVE ────── deploy: ANN index, ONNX export, runtime inference
```

The key design decision is that **EmbeddingStore is the universal interface**. Producers (mimic, autoencoders, pretrained models) write to it. Consumers (adapters, ANN, downstream models) read from it. This means you can swap the embedding engine without touching the adapter, or swap the adapter without retraining embeddings.

## Module Map

| Module            | Responsibility                         | Key Artifact                |
| ----------------- | -------------------------------------- | --------------------------- |
| `mana.encoders`   | Feature preprocessing                  | Fitted encoder (joblib)     |
| `mana.mimic`      | Set embedding via contrastive learning | Trained `MimicModel`        |
| `mana.embeddings` | Storage & streaming                    | `EmbeddingStore` (.pt file) |
| `mana.adapters`   | Task-specific heads                    | Task model + predictions    |
| `mana.serve`      | Deployment infrastructure              | ONNX model, ANN index       |

## What Flows Where

**Encoders → Mimic**: Encoders transform raw features (strings, floats) into tensors. These tensors become the elements of sets that mimic learns from. The `TabularElementEncoder` in mimic handles the categorical-embedding step itself, so you can also skip mana.encoders and go straight to mimic with pre-processed data.

**Mimic → EmbeddingStore**: After training, `MimicModel.encode_all()` produces an (N, D) tensor. `EmbeddingStore.from_mimic(model, dataset)` wraps it with IDs and metadata. The store is serialised to disk and can be loaded anywhere.

**EmbeddingStore → Adapters**: Adapters pull embeddings from the store. The recommender accepts pre-computed embedding vectors and learns lightweight projection layers to adapt them for retrieval (dot-product scoring) and ranking (MLP scoring). The forecast adapter uses a different path (LSTM on windowed time series), but the embedding concept is the same — a fixed representation fed into a task head.

**EmbeddingStore → Serve**: The ANN module indexes the embedding matrix for fast retrieval. `ANNFacade` picks the right backend (ScANN on Linux, KDTree on macOS). ModelManager exports models to ONNX for language-agnostic serving.

## Current State

- **Encoders**: Stable, complete API
- **Mimic**: Stable, well-tested, actively developed
- **Embeddings**: Stable (EmbeddingStore, SQLStreamingDataset)
- **Adapters**: Mixed — recommender is functional, forecast works but tied to internal DB, autoencoder is mid-refactor
- **Serve**: Stable, platform-aware
- **Data layer**: Being refactored from `data_loaders/` to `data/`
