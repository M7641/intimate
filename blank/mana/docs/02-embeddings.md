# Embeddings

## The Central Artifact

`EmbeddingStore` is the glue of the whole pipeline. It's a simple thing — a list of IDs, an (N, D) tensor, and some metadata — but being explicit about this interface is what lets producers and consumers evolve independently.

```python
from mana.embeddings import EmbeddingStore

# From mimic (the common path)
store = EmbeddingStore.from_mimic(model, dataset)

# From external embeddings (pretrained, another library, etc.)
store = EmbeddingStore.from_pretrained(numpy_array, ids=entity_ids)

# Direct construction
store = EmbeddingStore(ids=["a", "b", "c"], embeddings=tensor_3x64)
```

## What You Can Do With a Store

**Look things up** — by ID, not index position:

```python
store["customer_42"]           # → (D,) tensor
store[["c1", "c2", "c3"]]     # → (3, D) tensor
```

**Split for evaluation**:

```python
train, test = store.split(test_ratio=0.2, seed=42)
```

**Save / load** — torch serialisation, survives across sessions:

```python
store.save("embeddings.pt")
loaded = EmbeddingStore.load("embeddings.pt")
```

**Export** — to numpy for sklearn, to dict for lookups:

```python
store.to_numpy()  # → np.ndarray (N, D)
store.to_dict()   # → {id: tensor, ...}
```

## SQLStreamingDataset

For datasets too large to fit in memory, `SQLStreamingDataset` pages through a SQL table and yields (features, target) tensor pairs:

```python
from mana.embeddings import SQLStreamingDataset

dataset = SQLStreamingDataset(
    load_data=my_db_query_fn,
    sql_table_name="features",
    sql_table_schema="ml",
    order_by_column="id",
    target_column="label",
    features=["feat_1", "feat_2", "feat_3"],
    buffer_size=1_000,
)
```

It's an `IterableDataset` — use with a standard `DataLoader`. The `load_data` callable is pluggable so it works with any database connection layer.

## Design Notes

- **IDs travel with the data.** The #1 source of bugs in embedding workflows is misaligning IDs with rows. EmbeddingStore enforces `len(ids) == embeddings.shape[0]` at construction. The recent addition of `SetDataset(sets, ids=...)` extends this principle upstream into the training data.
- **Metadata is freeform.** The `metadata` dict is for provenance — what model produced these, what hyperparameters, what date. There's no schema enforcement, just `{"source": "mimic"}` by default.
- **`from_mimic` is the happy path.** It handles the DataLoader creation when you pass a SetDataset directly, extracts IDs automatically, and calls `encode_all()`. The older signature `from_mimic(model, dataloader, ids=ids)` still works for custom DataLoader configurations.
