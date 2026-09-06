# Serve

The serve module handles everything after training: indexing embeddings for fast retrieval, exporting models to portable formats, and running inference.

## ANN (Approximate Nearest Neighbours)

Once you have embeddings, the most common operation is "find the k most similar items". Brute-force search is O(N) per query — fine for hundreds, not for millions. ANN trades a tiny amount of accuracy for orders-of-magnitude speed.

### ANNFacade

The entry point is `ANNFacade`, which automatically picks the right backend for your platform:

```
Linux  → ScANN  (Google's library, fast, approximate)
macOS  → KDTree (scikit-learn, exact, portable)
```

Both expose the same interface:

```python
from mana.serve import ANNFacade

ann = ANNFacade(query_model=model)
ann.index(candidate_embeddings, identifiers=entity_ids)
results = ann.query(query_tensor, k=10)

ann.save_index("index/")
ann.load_index("index/")
```

### ScANN

Google's ScANN (Scalable Nearest Neighbors) uses a tree + asymmetric hashing approach:

1. **Tree partitioning** — splits the space into ~100 leaves, searches only the closest ~10
2. **Asymmetric hashing** — quantises candidates but keeps queries at full precision
3. **Rescoring** — refines the top candidates with exact distance

This gives sub-millisecond queries on million-scale datasets. Linux-only (C++ extension).

### KDTree

scikit-learn's KDTree. Exact results, works everywhere, reasonable performance up to ~100k items. Good for development and small-scale deployment.

## ONNX Export

PyTorch models can be exported to ONNX for serving outside Python (C++, Java, edge devices):

```python
from mana.serve import ModelManager

ModelManager.save_onnx(model, input_dim=64, file_path="model.onnx")
loaded = ModelManager.load_onnx("model.onnx")
```

The export uses dynamic batch dimensions so the same ONNX model handles single items and batches.

## ONNX Runtime Inference

For Python-based serving, `ORTModelRuntime` wraps ONNX Runtime:

```python
from mana.serve import ORTModelRuntime

runtime = ORTModelRuntime("model.onnx")
predictions = runtime.predict(input_tensor, input_dim=(1, 64))
```

Uses the CPU execution provider. For GPU inference, you'd modify the provider list — but for most embedding-based serving, the bottleneck is the ANN search, not the model inference.

## Deployment Pattern

A typical serving setup:

```
1. Train mimic → save model + EmbeddingStore
2. Export model to ONNX (optional, for non-Python serving)
3. Index EmbeddingStore with ANNFacade
4. At query time:
   - Encode query features → embedding
   - ANN search → top-k candidate IDs
   - (Optional) Ranking model → reorder candidates
   - Return results
```

## Open Questions

- GPU inference provider configuration
- Quantisation of ONNX models for edge deployment
- Periodic re-indexing strategy when embeddings are updated
