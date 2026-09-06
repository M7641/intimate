# Adapters

Adapters take embeddings and adapt them to a specific prediction task. The name is intentional — the embedding does the heavy lifting of representation learning; the adapter is a relatively thin layer that maps embeddings to an objective.

## Recommender

The recommender follows the classic two-stage architecture used at Google, YouTube, and most large-scale recommendation systems. Models accept **pre-computed embedding vectors** (e.g. from mimic/EmbeddingStore) and learn small task-specific projection layers — no internal feature encoding.

### The Idea

Retrieval and ranking are separate problems solved in sequence:

```
All candidates (millions)
  |
  v  RETRIEVAL ── fast, approximate, narrows to ~100 candidates
  |
  v  RANKING ──── slow, precise, scores the ~100 and returns top-k
  |
  v
Recommendations (10-20)
```

### Retrieval: Projection + Dot Product

Two independent projection networks map pre-computed query and candidate embeddings into a shared scoring space. Similar query-candidate pairs end up close together.

```
query_embedding (D,)  ──→ [Linear → ReLU → Linear] ──→ projected (P,)
                                                              |
                                                         dot product → score
                                                              |
candidate_embedding (D,) → [Linear → ReLU → Linear] ──→ projected (P,)
```

Each projection is a 2-layer MLP with ReLU. Two layers (not one) because the dot-product scoring provides no further nonlinearity — the projection IS the model's learnable capacity.

Training uses hard negative mining: for each query, the most similar-but-incorrect candidates are selected as negatives. The `UniqueQueryBatchSampler` ensures each batch has unique query IDs — without this, the same query appearing twice would create false negatives.

Key classes:
- `RetrievalTrainConfig` — dataclass centralising all training hyperparameters (architecture, optimizer, scheduler, hard negatives, temperature, curriculum, regularisation). Defaults reproduce the original behaviour. Supports curriculum learning (linear ramp of hard negatives over warmup epochs) and embedding collapse regularisation (uniformity + variance losses, gated by weight fields).
- `RetrievalModel` — orchestrates two projection layers, training loop, hard negative mining, metric evaluation. Constructor: `RetrievalModel(candidate_data, embedding_dim, config=RetrievalTrainConfig(), ...)`
- `RankingTrainConfig` — dataclass centralising all ranking hyperparameters (architecture, optimizer, scheduler, early stopping, loss). Configurable loss function (MSE/Huber/MAE), dropout, gradient clipping.
- `RankingModel` — pointwise ranker with projection + MLP regressor. Constructor: `RankingModel(embedding_dim, target_variable="labels", config=RankingTrainConfig())`. Early stopping, checkpointing, metrics (Pearson, Spearman, MAE).
- `InteractionDataset` / `InteractionDataLoader` — manages query-candidate pairs with pre-computed embeddings. Takes `embedding_dim` instead of field metadata

### Ranking

After retrieval narrows candidates to ~100, the ranker scores each pair precisely:

```
query_embedding (D,)     → [Linear] → projected (H,) ─┐
                                                       ├─→ [Linear → ReLU → Linear] → score
candidate_embedding (D,) → [Linear] → projected (H,) ─┘
```

`RankingModel` uses single linear projections per side (no ReLU — the regressor MLP already provides nonlinearity), concatenates the projections, and feeds them through a 2-layer MLP regressor with dropout. Constructor: `RankingModel(embedding_dim, target_variable="labels", config=RankingTrainConfig())`. Trained with configurable loss (MSE/Huber/MAE) on relevance scores. Supports early stopping, checkpointing, gradient clipping, and computes Pearson, Spearman, and MAE metrics during evaluation.

### Metrics

All standard retrieval metrics are implemented:
- `precision_at_k`, `recall_at_k`, `ndcg_at_k`
- `hit_rate_at_k`, `mean_reciprocal_rank`
- `coverage`, `diversity`

### Open Questions

- Listwise ranking vs pointwise — does our use case benefit from learning-to-rank approaches?
- Impact of embedding normalisation on retrieval quality
- Best fusion strategy for combining multiple feature embeddings (concat vs weighted vs late fusion)

See `docs/recommender.md` for research notes and references.

---

## Forecast (LSTM)

Time series forecasting using an LSTM architecture with the windowed approach.

### The Pipeline

```
Historical data (SQL) → WindowGenerator → sliding windows → LSTM → predictions
```

1. **Data loading**: Jinja2-templated SQL queries pull historical and dynamic features from a database
2. **Window generation**: `WindowGenerator` slices time series into input/target windows of configurable width (e.g. 40 weeks history → 12 weeks forecast)
3. **Feature processing**: Categorical features get embedding layers, numeric features get normalised, static features get sliced per entity
4. **LSTM model**: 64-unit LSTM → Dense(128, tanh) → Dense(1, softplus). Trained with a custom loss function, Lion optimiser with exponential decay
5. **Output**: Predictions are formatted and written back to the database

### Key Components

- `WindowGenerator` — the workhorse. Handles encoding, windowing, train/test splitting, and future date spine generation
- `LSTM` — TensorFlow/Keras model (note: this is the one module that uses TF, not PyTorch)
- `Naive` — baseline model that repeats the last observed value
- `forecast_data.py` — SQL query functions for historical and dynamic features

### Current State

Functional but tightly coupled to the internal database layer (`bhg.connect.DBObj`). The SQL templates and data loading are specific to the current deployment environment. The modelling code (WindowGenerator, LSTM) is more general.

### Open Questions

- Migration from TensorFlow to PyTorch for consistency with the rest of mana
- Decoupling the data loading from the specific database layer
- Whether mimic embeddings could serve as input features to the forecast model (entity embeddings as static features)

---

## Autoencoder

Autoencoders compress high-dimensional input into a lower-dimensional latent space, then reconstruct the original. The latent representation is an embedding.

```
Input → Encoder → latent (embedding) → Decoder → Reconstruction
                    ↓
              use this as the entity embedding
```

### Components (in refactoring)

- `AutoEncoder` — base autoencoder
- `CollectionEncoder` — encodes collections of items
- `CombinedEncoders` — combines multiple encoders
- `RelationalEncoder` — handles relational data structures

### Current State

Mid-refactor from `models/autoencoder/` to `adapters/autoencoder/`. The idea is that autoencoders are an alternative embedding producer to mimic — they both produce EmbeddingStore artifacts, but use different learning objectives (reconstruction vs contrastive).

---

## SimpleModel

A minimal linear regression head for quick experiments.

```python
from mana.adapters.simple_model import SimpleModel

model = SimpleModel(input_dim=64)
model.train_model(dataloader)
predictions = model.predict(embeddings)
```

Single linear layer, MSE loss, Adam optimiser. Useful as a baseline or for sanity-checking that embeddings carry signal.
