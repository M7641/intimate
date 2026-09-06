# Mana

Embedding-centric machine learning library.

**Pipeline:** encode features → create embeddings (mimic) → adapt to task → serve.

Mana provides reusable building blocks for ML workflows: feature encoders (categorical, numeric, text), contrastive set embeddings (mimic), an EmbeddingStore bridge that decouples producers from consumers, predictive adapters (two-tower recommenders, forecasting, ranking), and serving infrastructure (ANN search, ONNX export).

## Install

Requires Python >=3.12.

```bash
uv add git+https://github.com/nimbus-labs/nimbus-monorepo.git@main#subdirectory=mana
```

## Quick start

```python
import torch
from mana import mimic
from mana.embeddings import EmbeddingStore

# Variable-size sets with entity IDs
sets = [torch.randn(n, 4) for n in [3, 5, 2, 7]]
dataset = mimic.SetDataset(sets, ids=["a", "b", "c", "d"])
loader = mimic.set_dataloader(dataset, batch_size=2)

# Contrastive model
model = mimic.MimicModel(
    encoder=mimic.SetTransformerEncoder(input_dim=4, dim=32, output_dim=16, num_heads=4),
    projector=mimic.MLPProjector(16, 16, 16),
    augmentation=mimic.Compose([mimic.FeatureMask(p=0.15), mimic.FeatureNoise(std=0.1)]),
    loss_fn=mimic.NTXentLoss(temperature=0.1),
)

# Train
result = mimic.ContrastiveTrainer(model, mimic.TrainConfig(epochs=50)).fit(loader)

# Embed + store
store = EmbeddingStore.from_mimic(model, dataset)
store["a"]                                        # (16,) lookup by ID
train_store, test_store = store.split(seed=42)    # train/test split
store.save("embeddings.pt")                       # serialise
```

## CLI

```bash
uv run mana examples --help  # list all examples
```

## Examples

All examples are self-contained — they generate synthetic data, train a model, and print results. No external data or GPU needed.

```bash
cd mana
uv run mana examples deep-sets
```

| Command                                      | What it demonstrates                                                                                                                                                |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `uv run mana examples deep-sets`             | Deep Sets encoder on synthetic clustered sets. Shows permutation-invariant encoding via element MLP + mean aggregation.                                             |
| `uv run mana examples set-transformer`       | Set Transformer encoder on the same data. Compares attention-based encoding (SAB/PMA) against Deep Sets.                                                            |
| `uv run mana examples polars-tabular`        | Real-world-ish scenario: customer transaction DataFrame with mixed categorical + continuous features. Uses `TabularElementEncoder` for embedding lookups.           |
| `uv run mana examples embedding-regression`  | Full pipeline: generate entities with structure-dependent targets → train mimic → store embeddings → train regressor → evaluate R² against a mean-pooling baseline. |
| `uv run mana examples retrieval-recommender` | Retrieval recommender: synthetic clustered embeddings → projection towers → hard negative training → precision/recall vs random baseline.                           |
| `uv run mana examples ranking-recommender`   | Ranking recommender: synthetic embeddings with relevance scores → projection + MLP regressor → MSE and Pearson correlation evaluation.                              |

Each example prints a training progress bar and evaluation metrics to the terminal. The embedding-regression example also saves an `entity_embeddings.pt` file you can inspect:

```python
from mana.embeddings import EmbeddingStore

store = EmbeddingStore.load("entity_embeddings.pt")
print(len(store), store.dim)   # 600 entities, 64 dimensions
print(store[0])                # embedding for entity 0
```

Source code for all examples lives in [`mana/examples/`](mana/examples/).

## Modules

| Module            | What it does                                                      |
| ----------------- | ----------------------------------------------------------------- |
| `mana.encoders`   | Feature preprocessing (LabelEncoder, NumericEncoder, TextEncoder) |
| `mana.mimic`      | Contrastive learning for set-structured data                      |
| `mana.embeddings` | EmbeddingStore + SQL streaming dataset                            |
| `mana.adapters`   | Task heads (recommender, forecast, simple regressor)              |
| `mana.serve`      | ANN search (ScANN/KDTree), ONNX export, runtime inference         |

## Docs

See [llms.txt](llms.txt) for full API reference and [docs/](docs/) for guides:

- [Architecture overview](docs/00-architecture.md)
- [Encoders](docs/01-encoders.md)
- [Embeddings](docs/02-embeddings.md)
- [Adapters](docs/03-adapters.md)
- [Serve](docs/04-serve.md)
- [Future: Temporal embeddings](docs/05-future-temporal-embeddings.md)
- [Mimic deep dives](docs/mimic/)

## Future

1. How will we do the data pipelines?
   1. The encoding logic would have to be attached to an instance of a model. Or at least connected in some way.
2. What would it look like to use a set transformer to directly just train a regressor or a binary classifier?
3. What can we do for explainability in this kind of framework? Have an explainable model that predicts the first output again?
4. https://github.com/upgini/upgini
5. https://github.com/seldonio/alibi
6. https://github.com/feature-engine/feature_engine
7. https://torchdrift.org/
8. https://captum.ai/
9. https://deepchecks.com/
10. https://mlflow.org/docs/latest/ml/
11. Quantisation to reduce the memory cost of recommender models. Also investigate if this is a way to optimise inference speed, can we get something that runs faster than an XGBoost in a quote optimisation process, for example. The new TurboQuant suggests that there are big gains to be had on the memory side at least.
