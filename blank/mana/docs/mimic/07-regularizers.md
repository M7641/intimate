# Regularizers: Encouraging Embedding Uniqueness

## Why Contrastive Loss Alone Isn't Enough

NT-Xent pulls positive pairs together and pushes negatives apart. That sounds like everything you need for unique embeddings. But there are three blind spots:

1. **It only repels within a batch.** If two samples never appear in the same batch, they get zero repulsion signal. With 1M items and batch size 256, most pairs never meet.

2. **Dimensional collapse is invisible to it.** Embeddings can satisfy the contrastive objective while living in a low-dimensional subspace. If only 8 of your 128 dimensions carry signal, you're wasting 94% of your capacity — but the loss doesn't care, because cosine similarity normalises away the dead dimensions.

3. **The gradient vanishes for well-separated pairs.** Once two embeddings are far enough apart, the softmax denominator makes their gradient negligible. There's no incentive to keep spreading.

The result: embeddings that *technically* satisfy the contrastive objective but cluster in pockets, waste dimensions, and can't distinguish as many inputs as the dimensionality allows.

Three regularizers from the literature fix these blind spots, each targeting a specific failure mode.

---

## 1. Uniformity Loss — "Spread Out on the Sphere"

**Paper**: Wang & Isola, "Understanding Contrastive Representation Learning through Alignment and Uniformity" (ICML 2020)

### The Problem It Solves

Contrastive loss pushes negatives apart, but only within the batch. Uniformity loss provides a direct, global pressure to spread embeddings across the entire hypersphere.

### The Math

```
L_uniform = log( mean( exp(-t × ||zᵢ - zⱼ||²) ) )    for all i ≠ j
```

This is the **Gaussian potential kernel** — it penalises any pair of points that are close together. The log-mean-exp form means one close pair dominates the loss (since exp is largest for small distances), so the loss focuses on eliminating the worst clusters first.

**t** (default 2.0) controls sensitivity to distance. Higher t penalises nearby pairs more aggressively.

### Where It Operates

On **z** (projected, L2-normalised embeddings). This is a hypersphere metric — it measures how uniformly points cover S^(D-1).

### In Code

```python
from mana.mimic.losses import uniformity_loss

z = model.embed(x, mask)  # projected embeddings
loss = uniformity_loss(z, t=2.0)
```

Or integrated into the model (preferred):

```python
model = MimicModel(
    ...,
    uniformity_weight=0.1,  # added to total loss
)
```

### Connection to Evaluation

Mimic already has `mimic.uniformity()` as an **evaluation metric** (see [05-evaluation](05-evaluation.md)). The regularizer is the **differentiable training version** of the same idea. During training, you optimise it; during evaluation, you measure it.

---

## 2. Variance Loss — "Keep All Dimensions Alive"

**Paper**: Bardes, Ponce & LeCun, "VICReg" (ICLR 2022)

### The Problem It Solves

**Dimensional collapse**: some embedding dimensions go constant across all inputs. If dimension 47 is always 0.3 regardless of input, it carries zero information. Effectively, your 128-dimensional embedding is actually, say, 64-dimensional — halving the number of distinct points you can represent.

This is invisible to contrastive loss because cosine similarity normalises away constant dimensions. The model can cheat by collapsing half its dimensions and still achieve low contrastive loss.

### The Math

```
L_var = mean( relu(γ - std(zⱼ)) )    across dimensions j
```

For each dimension j, compute the standard deviation across the batch. If it's below the threshold γ (default 1.0), apply a hinge penalty. If it's above γ, no penalty.

This is a **hinge loss** — it doesn't demand large variance, just *sufficient* variance. Once a dimension is "alive" (std ≥ γ), it's left alone.

### Where It Operates

On **h** (encoder output, pre-projection). Two reasons:

1. `model.encode()` returns **h**, and this is what you use as your embedding ID. Regularising h ensures the dimensions you actually use at inference are healthy.
2. L2-normalised embeddings (like z) have mathematically constrained variance. A hinge loss on normalised vectors would be fighting the normalisation.

### In Code

```python
from mana.mimic.losses import variance_loss

h = encoder(x, mask)  # encoder output, pre-projection
loss = variance_loss(h, gamma=1.0)
```

Or integrated:

```python
model = MimicModel(
    ...,
    variance_weight=0.1,
)
```

### Diagnosing Dimensional Collapse

To check if you need this, inspect the per-dimension standard deviation of your embeddings:

```python
embeddings = model.encode(eval_x, eval_mask)
stds = embeddings.std(dim=0)
print(f"Dead dimensions (std < 0.01): {(stds < 0.01).sum()}/{stds.shape[0]}")
print(f"Min std: {stds.min():.4f}, Median std: {stds.median():.4f}")
```

If more than ~10% of dimensions are dead, turn on variance regularization.

---

## 3. Covariance Loss — "Make Dimensions Independent"

**Paper**: Bardes, Ponce & LeCun, "VICReg" (ICLR 2022)

### The Problem It Solves

Even if all dimensions are alive (variance loss handles that), they might be **correlated** — carrying redundant information. If dimension 12 is always 0.95× dimension 37, they're effectively the same dimension. You're paying for 128 dimensions but only getting the information capacity of fewer.

### The Math

```
C = (Z - mean(Z))ᵀ(Z - mean(Z)) / (n - 1)      covariance matrix

L_cov = sum(Cᵢⱼ² for i ≠ j) / D                  off-diagonal penalty
```

Compute the covariance matrix of the embeddings. Square all off-diagonal entries and sum them. Diagonal entries (each dimension's variance with itself) are excluded — that's the variance loss's job.

### Why Squared Off-Diagonal, Not Absolute?

Squaring penalises strong correlations quadratically — a correlation of 0.8 contributes 16× more loss than a correlation of 0.2. This focuses the regularizer on eliminating the worst redundancies first.

### Where It Operates

On **h** (encoder output, pre-projection), for the same reasons as variance loss — you want the inference embeddings to be decorrelated.

### In Code

```python
from mana.mimic.losses import covariance_loss

h = encoder(x, mask)
loss = covariance_loss(h)
```

Or integrated:

```python
model = MimicModel(
    ...,
    covariance_weight=0.04,
)
```

---

## How They Compose

Each regularizer targets a different failure mode. They're independent and complementary:

| Regularizer | What it ensures | Without it |
|-------------|----------------|------------|
| **Uniformity** | Points spread across the sphere | Clusters and gaps in embedding space |
| **Variance** | All dimensions are "alive" | Some dims collapse → effectively lower D |
| **Covariance** | Dimensions carry independent info | Redundant dims → wasted capacity |
| **Reconstruction** (existing) | Input info preserved | Two different inputs could map to same point |

Together: **full-rank embeddings uniformly spread on the hypersphere, with input information preserved.** This is the strongest guarantee of uniqueness short of explicit collision detection.

### A Visual Analogy

Think of packing oranges in a crate:

- **Uniformity** = oranges spread evenly throughout the crate (not piled in one corner)
- **Variance** = the crate uses all three spatial dimensions (oranges aren't flattened into a 2D plane)
- **Covariance** = the crate's axes are independent (oranges spread along x, y, z separately, not along a single diagonal)

---

## Sphere Packing: Why Dimension Matters

On S^(D-1) (the unit sphere in D dimensions), the number of distinguishable points grows **exponentially** with D:

| Embedding dim D | Approx. distinguishable points (cos_sim ≥ 0.99) |
|-----------------|--------------------------------------------------|
| 32 | ~10⁴ |
| 64 | ~10⁸ |
| 128 | ~10¹⁶ |
| 256 | ~10³² |

**Rule of thumb**: D ≥ 2 × log₂(N) for N data points. For 1M items, you need D ≥ 40.

But this is the *theoretical maximum*. Without regularizers, dimensional collapse and correlation can reduce your effective dimensionality far below D. The regularizers ensure your embeddings **actually use** all the dimensions you're paying for.

---

## Recommended Weights

All three regularizers default to weight 0.0 (disabled), so they're fully backwards compatible.

| Config | uniformity | variance | covariance | reconstruction | When to use |
|--------|-----------|----------|------------|----------------|-------------|
| **Conservative** | 0.05 | 0.0 | 0.0 | 0.1 | First experiment, just add some spread |
| **Recommended** | 0.1 | 0.1 | 0.04 | 0.1 | General-purpose starting point |
| **Aggressive** | 0.5 | 0.25 | 0.1 | 0.05 | Max uniqueness, e.g. large-scale ID generation |

Start with the recommended config and adjust based on evaluation metrics:

```python
model = MimicModel(
    encoder=encoder,
    projector=projector,
    augmentation=augmentation,
    loss_fn=loss_fn,
    decoder=decoder,
    reconstruction_weight=0.1,
    uniformity_weight=0.1,
    variance_weight=0.1,
    covariance_weight=0.04,
)
```

### Tuning Heuristics

- **Uniformity too strong** → embeddings become perfectly uniform but lose class structure. Reduce weight.
- **Variance too strong** → forces dimensions to have high variance even when the data doesn't support it. Can cause noise amplification. Reduce weight or lower γ.
- **Covariance too strong** → over-decorrelation can fight useful feature correlations. Reduce weight.

A good diagnostic: if contrastive loss stops decreasing while regularizer losses keep decreasing, the regularizers are overpowering the main objective. Scale them back.

---

## What Doesn't Change

These regularizers are wired into `MimicModel.forward()`. Everything else stays the same:

- The **trainer** calls `model(x, mask)` and gets back a scalar loss — no trainer changes needed
- **Evaluation** metrics are unaffected
- `model.encode()` and `model.embed()` work exactly as before
- All existing configurations with default weights (0.0) produce identical results
