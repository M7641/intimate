# Loss Functions: Contrastive Learning Objectives

Mimic provides two loss functions that cover the main contrastive learning scenarios.

## The Fundamental Problem: Representational Collapse

If you just minimise "pull positive pairs together," the model will cheat: **collapse every embedding to the same point**. If everything maps to `[0, 0, ..., 0]`, all positive pairs have distance 0. Loss = 0. "Perfect."

Obviously useless. Every loss function must include a mechanism to **prevent collapse** — to ensure embeddings spread out and carry information.

| Loss | Collapse Prevention |
|------|-------------------|
| NTXent | Explicit negative pairs push embeddings apart |
| SupCon | Same, plus label-aware positive pairs |

---

## 1. NTXentLoss (Normalized Temperature-scaled Cross-Entropy)

**Paper**: Chen et al., "SimCLR" (ICML 2020)

The workhorse of contrastive learning. Also called **InfoNCE** in some contexts.

### Intuition

Given a batch of B samples, each with two augmented views, you have 2B vectors. For each anchor vector, there's exactly **1 positive** (the other view of the same sample) and **2B-2 negatives** (all other vectors). NT-Xent is just softmax classification: "which of the 2B-1 other vectors is my positive?"

### The Math

For anchor i with positive j:

```
L_i = -log( exp(sim(zi, zj) / t) / Sum_{k!=i} exp(sim(zi, zk) / t) )
```

This is literally **cross-entropy** where the similarity matrix (divided by temperature) serves as logits and the positive pair index is the target class.

### Temperature t

Temperature controls how "peaked" the softmax distribution is:

- **Low t (e.g. 0.05)**: Very peaked — stronger gradients from hard negatives but can be unstable.
- **High t (e.g. 1.0)**: Flat distribution — more stable but less discriminative.
- **Sweet spot (0.07-0.2)**: Where most papers settle. Mimic defaults to 0.1.

### Learnable Temperature

Rather than hand-tuning t, you can let the model learn it:

```python
loss_fn = mimic.NTXentLoss(temperature=0.1, learnable=True)
```

The temperature is parameterised in log-space and clamped to [0.01, 1.0].

### Hard Negative Mining Strategies

Not all negatives are equally useful. Easy negatives (already well-separated) provide little gradient signal. Hard negatives (similar to the anchor) are more informative.

`NTXentLoss` supports four strategies via the `strategy` parameter:

#### `"none"` (default) — Standard NT-Xent

All negatives weighted equally via softmax. Simple, robust baseline.

```python
loss_fn = mimic.NTXentLoss(temperature=0.1)
```

#### `"debiased"` — Corrects False Negatives

In a finite batch, some "negatives" might actually be similar to the anchor. The debiased correction (Chuang et al., NeurIPS 2020) subtracts out the estimated false-negative contribution:

```python
loss_fn = mimic.NTXentLoss(
    temperature=0.1,
    strategy="debiased",
    tau_plus=0.1,  # estimated positive class probability
)
```

Best theoretical foundation. Use when your dataset may contain similar samples across different batches.

#### `"hardest"` — Most Aggressive

Only uses the single hardest negative per anchor. Fastest convergence but can be unstable with noisy data.

```python
loss_fn = mimic.NTXentLoss(temperature=0.1, strategy="hardest")
```

#### `"semi-hard"` — Balanced

Only uses negatives harder than the positive but within a margin. Falls back to all negatives if none qualify.

```python
loss_fn = mimic.NTXentLoss(
    temperature=0.1,
    strategy="semi-hard",
    margin=0.1,
)
```

### Strengths and Weaknesses

- **Strength**: Simple, well-understood, strong baseline
- **Weakness**: Benefits from large batch sizes (more negatives = better gradients)

---

## 2. Supervised Contrastive Loss (SupCon)

**Paper**: Khosla et al., "Supervised Contrastive Learning" (NeurIPS 2020)

### When You Have Labels

Standard contrastive learning treats each sample as its own class: (view_1, view_2) of the same sample are the only positives. But if you know that samples A, B, C all belong to the same class, why not treat all of them as positives for each other?

SupCon does exactly this:

```
Without labels: only (view_1_i, view_2_i) are positive pairs
With labels:    all (view_j, view_k) where label[j] == label[k] are positive pairs
```

### Self-Supervised Fallback

When `labels=None`, SupCon falls back to standard NT-Xent behaviour. This means you can use the same loss function for both supervised and self-supervised settings.

### In Mimic

```python
loss_fn = mimic.SupConLoss(temperature=0.1)

# During training with labels:
loss = model(x, mask, labels=labels_tensor)

# Or without labels (self-supervised fallback):
loss = model(x, mask)
```

---

## Decision Guide

```
Do you have labels?
  Yes -> SupConLoss
  No  -> NTXentLoss (start with strategy="none")
           -> If loss plateaus, try strategy="debiased"
```

## A Note on Projectors

All these losses operate on **projected** embeddings (the output of the projector), not the encoder embeddings directly. After training, you throw away the projector and use the encoder output.

Why? The projector acts as a "buffer" — contrastive losses can discard information that isn't useful for the contrastive task but IS useful for downstream tasks. By operating on projected embeddings, the loss only distorts the projector's space, leaving the encoder's representation richer.

This was a key finding from SimCLR: using a 2-layer MLP projector improved downstream accuracy by ~10% compared to using the encoder output directly for the loss.
