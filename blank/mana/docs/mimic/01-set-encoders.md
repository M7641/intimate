# Set Encoders: Making Neural Networks Respect Set Structure

## The Problem With Sets

Most neural networks expect **fixed-size, ordered** input: an image is always 224x224x3, a sentence is a sequence with positions. But sets are:

- **Variable-size**: one customer has 5 transactions, another has 500
- **Unordered**: the same set in any order should produce the same output (permutation invariance)

If you just flatten a set into a vector, you've imposed an arbitrary ordering. If you truncate to a fixed length, you lose information. You need an architecture that's **inherently permutation-invariant**.

## Deep Sets: The Simplest Correct Approach

**Paper**: Zaheer et al., "Deep Sets" (NeurIPS 2017)

Deep Sets is based on a beautiful theoretical result: **any permutation-invariant function on sets can be decomposed as:**

```
f({x₁, x₂, ..., xₙ}) = ρ(Σᵢ φ(xᵢ))
```

where `φ` (phi) encodes individual elements and `ρ` (rho) transforms the aggregated result.

### Architecture

```
Input set: {x₁, x₂, ..., xₙ}     (each xᵢ is a D-dimensional vector)
           │
     ┌─────┼─────┐
     ▼     ▼     ▼
   φ(x₁) φ(x₂) φ(xₙ)             Element encoder: shared MLP applied per-element
     │     │     │
     └──┬──┘     │
        ▼        ▼
     aggregate (mean/sum/max)       Permutation-invariant pooling
           │
           ▼
         ρ(·)                       Post-aggregation MLP
           │
           ▼
       embedding                    Fixed-size output vector
```

### Why It Works

Since `φ` is applied independently to each element, and sum/mean/max don't depend on order, the whole pipeline is permutation-invariant by construction. There's no clever trick — it's just architecturally incapable of caring about order.

### In Mimic

```python
model = mimic.DeepSetsEncoder(
    element_encoder=mimic.MLPElementEncoder(input_dim=10, hidden_dim=64, output_dim=64),
    aggregator=mimic.MeanAggregator(),
    rho=nn.Sequential(nn.Linear(64, 128), nn.ReLU(), nn.Linear(128, 128)),
    output_dim=128,
)
```

**When to use Deep Sets**: Fast, simple, works well for small-to-medium sets. Use as your starting point. If performance plateaus, try the Set Transformer.

### Mean vs Sum vs Max Aggregation

- **Mean**: Most common default. Normalises by set size, so a set of 5 and a set of 500 contribute equally. Good when set size doesn't carry information.
- **Sum**: Set size matters — a customer with more transactions should have a "larger" representation. Can cause magnitude issues with very different set sizes.
- **Max**: Takes the "most activated" feature across elements. Good for detecting the presence of specific patterns. Loses information about frequency.

All three are mask-aware in Mimic: padded positions (where `mask=False`) are correctly excluded.

## Set Transformer: Attention-Based Set Encoding

**Paper**: Lee et al., "Set Transformer" (ICML 2019)

Deep Sets processes each element independently before aggregation — elements never "talk to" each other. The Set Transformer fixes this with **self-attention**, allowing elements to interact.

### Building Blocks

The Set Transformer is built from four components, each one building on the last:

#### MAB — Multihead Attention Block

The fundamental building block. Given queries X and keys/values Y:

```
MAB(X, Y) = LayerNorm(H + FeedForward(H))
where H = LayerNorm(X + MultiheadAttention(X, Y, Y))
```

This is essentially a standard Transformer block.

#### SAB — Set Attention Block

Self-attention over the set: `SAB(X) = MAB(X, X)`

Each element attends to every other element. This is how elements "communicate" — element xᵢ can learn that it's similar to xⱼ, or that xₖ is an outlier.

**Complexity**: O(n²) where n is set size. Fine for sets up to ~500 elements.

#### ISAB — Induced Set Attention Block

The key efficiency innovation. Instead of n² attention, uses m learnable **inducing points** to reduce complexity to O(nm):

```
ISAB(X) = MAB(X, MAB(I, X))
```

Step 1: Inducing points I attend to the set (summarise it into m vectors)
Step 2: Set elements attend to the inducing point summaries

Think of inducing points as "summary slots" — the model learns to compress the set into m summarisations, then each element reads from those summaries instead of attending to every other element directly.

**Complexity**: O(nm) where m << n. Set m = 8-32 in practice.

#### PMA — Pooling by Multihead Attention

Aggregates the set into a fixed-size output using learnable seed vectors:

```
PMA(X) = MAB(S, X)
```

Instead of mean/max pooling, the model **learns** how to aggregate by having seed vectors attend to the set. Much more expressive than fixed pooling — the model can learn to focus on the most important elements.

### Full Architecture

```
Input set: (B, N, D_in)
      │
  Linear projection ──→ (B, N, dim)
      │
  SAB/ISAB stack ──→ (B, N, dim)      Elements interact via attention
      │
  PMA ──→ (B, 1, dim)                 Learned aggregation
      │
  Linear output ──→ (B, output_dim)
```

### In Mimic

```python
model = mimic.SetTransformerEncoder(
    input_dim=10,
    dim=64,
    output_dim=128,
    num_heads=4,
    num_layers=2,
    num_inducing_points=8,   # Use ISAB; set to None for SAB (full attention)
    dropout=0.0,
)
```

### A Subtle Detail: Mask Inversion

PyTorch's `MultiheadAttention` uses `key_padding_mask` where **True means "ignore this position"**. But Mimic's convention (and the more intuitive one) is **True = valid, False = padded**. The Set Transformer inverts the mask at its entry point:

```python
kpm = ~mask if mask is not None else None
```

This is a one-liner but a common source of bugs. Mimic handles it so you don't have to.
