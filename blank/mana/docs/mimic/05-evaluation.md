# Evaluation: How to Know If Your Embeddings Are Good

Training loss going down is necessary but not sufficient. A model could overfit to the contrastive objective while producing useless embeddings. You need independent metrics that measure **embedding quality** for downstream tasks.

Mimic provides five evaluation metrics, each measuring a different aspect of quality.

---

## 1. kNN Accuracy — "Can Neighbours Predict Labels?"

```python
acc = mimic.knn_accuracy(train_embeddings, train_labels, test_embeddings, test_labels, k=10)
```

### What It Measures

Given test embeddings, finds each one's k nearest neighbours among training embeddings (by cosine similarity), and classifies by majority vote. The accuracy tells you how well the embedding space preserves class structure.

### Why It's the Go-To Metric

kNN accuracy is the **standard evaluation** in contrastive learning papers because:

1. **No learning involved**: Unlike a linear probe, kNN has no parameters to train. It purely measures the geometry of the embedding space.
2. **Fast**: Just a matrix multiply + top-k.
3. **Intuitive**: "If similar things are close together, kNN should classify well."

### Interpretation

- **Random chance** = 1/num_classes (e.g., 20% for 5 classes)
- **Good contrastive model** = 70-95% (depends on task difficulty)
- **Perfect** = 100% (all same-class embeddings cluster together with no overlap)

If kNN accuracy is near chance, your embeddings aren't separating classes. Go back and check augmentations, training duration, or try a different loss.

### Choice of k

- **k=1**: Most sensitive to local structure, noisy
- **k=5-20**: Typical range, balances locality and robustness
- **k=N**: Meaningless (always predicts the majority class)

---

## 2. Linear Probe — "How Much Information Is In There?"

```python
acc = mimic.linear_probe(train_embeddings, train_labels, test_embeddings, test_labels, epochs=100)
```

### What It Measures

Trains a simple linear classifier (one `nn.Linear` layer) on **frozen** embeddings, then measures classification accuracy. The embeddings don't change — only the linear layer learns.

### Why It Matters

This answers: "Is the class information **linearly separable** in the embedding space?"

A linear classifier can only draw straight decision boundaries (hyperplanes). If it achieves high accuracy, the contrastive model has arranged classes into **linearly separable clusters** — the strongest form of useful representation.

### kNN vs Linear Probe

|                        | kNN                           | Linear Probe               |
| ---------------------- | ----------------------------- | -------------------------- |
| **Learns parameters?** | No                            | Yes (one linear layer)     |
| **Decision boundary**  | Non-linear (Voronoi cells)    | Linear (hyperplanes)       |
| **What it tests**      | Local neighbourhood structure | Global linear separability |
| **Use when**           | Quick sanity check            | Rigorous evaluation        |

You can have high kNN accuracy but low linear probe accuracy if classes form complex, non-linear clusters. High linear probe accuracy is a stronger signal.

---

## 3. Alignment — "Are Positive Pairs Close?"

```python
align = mimic.alignment(z1, z2, alpha=2.0)
```

### What It Measures

**Paper**: Wang & Isola, "Understanding Contrastive Representation Learning through Alignment and Uniformity" (ICML 2020)

Alignment measures the average distance between positive pairs on the unit hypersphere:

```
alignment = E[ ||z₁ᵢ - z₂ᵢ||^α ]
```

where z₁ᵢ and z₂ᵢ are the two views of sample i, both L2-normalised.

**Lower is better**. Zero means all positive pairs are identical.

### When to Use

Alignment tells you if the **invariance** objective is working. If alignment is high, the model hasn't learned to ignore your augmentations — the two views of the same sample are being mapped to different places.

---

## 4. Uniformity — "Are Embeddings Spread Out?"

```python
unif = mimic.uniformity(embeddings, t=2.0)
```

### What It Measures

How uniformly embeddings are distributed on the unit hypersphere:

```
uniformity = log E[ exp(-t × ||zᵢ - zⱼ||²) ]
```

Averaged over all pairs (excluding self-pairs).

**Lower (more negative) is better**. A perfectly uniform distribution on the sphere gives the most negative value.

### Why Alignment + Uniformity Together

Wang & Isola (2020) proved that a good contrastive representation optimises **both** alignment and uniformity. These two metrics decompose what the loss function is trying to do:

- **Alignment**: Pull positive pairs together (invariance)
- **Uniformity**: Spread all embeddings evenly (information preservation)

If alignment is good but uniformity is bad → embeddings are clustered but collapsed (many samples map to the same point)

If uniformity is good but alignment is bad → embeddings are spread out but positive pairs aren't close (augmentations too aggressive)

Both good → healthy contrastive representation.

---

## 5. Silhouette Score — "How Clean Are the Clusters?"

```python
sil = mimic.silhouette(embeddings, labels)
```

### What It Measures

A classic clustering quality metric. For each sample:

1. **a(i)** = mean distance to same-cluster samples
2. **b(i)** = mean distance to nearest _different_ cluster

```
silhouette(i) = (b(i) - a(i)) / max(a(i), b(i))
```

Range: [-1, 1]

- **+1**: Sample is far from other clusters and close to its own (perfect)
- **0**: Sample is on the boundary between clusters
- **-1**: Sample is closer to a different cluster than its own (misassigned)

### In Mimic

Uses cosine distance (1 - cosine_similarity) and is implemented in pure PyTorch — no sklearn dependency. This means it works on GPU tensors directly.

### Interpretation

- **> 0.5**: Strong cluster structure
- **0.25 - 0.5**: Moderate structure, some overlap
- **< 0.25**: Weak structure, clusters are fuzzy
- **< 0**: Embeddings aren't separating classes at all

---

## The CLI Evaluation Output

When you run `mimic quickstart`, the evaluation section at the end shows:

```
  ▸ Embedding Quality
  shape    [400, 128]
  kNN@10   97.8%                accuracy
  silhou   0.847                silhouette score [-1, 1]
  unifrm   -3.421               uniformity (lower = more spread)
  within   0.891                avg cosine sim (same cluster)
  across   0.023                avg cosine sim (diff cluster)
  gap      0.868                separation (within - across)

  ✔ Same-cluster objects are more similar — contrastive learning works!
```

The **within/across/gap** metrics are specific to the quickstart demo: they compute the average cosine similarity within clusters vs across clusters. A large gap means the model has clearly separated the clusters.

---

## Evaluation Workflow

After training, a thorough evaluation looks like:

```python
# Get embeddings
embeddings = model.encode(eval_batch.x, eval_batch.mask)

# Quick check: are clusters separated?
sil = mimic.silhouette(embeddings, labels)

# Standard benchmark: kNN and linear probe
knn = mimic.knn_accuracy(train_emb, train_lab, test_emb, test_lab, k=10)
lp = mimic.linear_probe(train_emb, train_lab, test_emb, test_lab)

# Diagnostic: alignment and uniformity
# (Requires paired views — run two augmented forward passes)
model.train()
z1 = model.embed(x, mask)
z2 = model.embed(x, mask)  # Different augmentation due to randomness
align = mimic.alignment(z1, z2)
unif = mimic.uniformity(z1)
```

### Red Flags

| Symptom                             | Likely Cause                                             |
| ----------------------------------- | -------------------------------------------------------- |
| kNN ≈ random chance                 | Training failed or too few epochs                        |
| High kNN, low linear probe          | Non-linear cluster structure; may be fine for many tasks |
| Very high alignment, bad uniformity | Collapse — all embeddings in same region                 |
| Bad alignment, good uniformity      | Augmentations too aggressive; views are too different    |
| Silhouette < 0                      | Something is wrong — start debugging                     |
