# Augmentations: Teaching the Model What Doesn't Matter

## Why Augmentations Are Critical

Augmentations are the **most important design choice** in contrastive learning — more important than the encoder architecture or loss function. Here's why:

The contrastive objective learns: "these two views came from the same source." What the model learns as a "good representation" is entirely determined by **what varies between views**.

- If you only add noise → the model learns to be robust to noise (but nothing else)
- If you only drop elements → the model learns that individual elements aren't essential
- If you do both → the model learns representations robust to *both*

The augmentation pipeline defines what information the model **must preserve** (things that are the same across views) versus what it can **ignore** (things that differ across views).

## Set-Specific Augmentations

Standard augmentations (crop, rotate, colour jitter) don't apply to set data. Mimic provides three augmentations designed specifically for sets:

### FeatureNoise — Gaussian Perturbation

```python
mana.mimic.FeatureNoise(std=0.1)
```

Adds Gaussian noise to every feature value:

```
x_aug = x + N(0, std²)
```

**What it teaches**: The exact numerical values don't matter — the model should capture the general pattern, not memorise precise numbers.

**Analogy**: Like adding slight colour jitter to an image — the "meaning" shouldn't change just because values shift slightly.

**Important**: Only active during training. In eval mode, passes input through unchanged.

### FeatureMask — Random Feature Zeroing

```python
mana.mimic.FeatureMask(p=0.15)
```

Randomly zeros out entire feature dimensions with probability p. The same features are masked for all elements in a set (broadcast shape `(B, 1, D)`).

```
features: [salary, age, tenure, region, ...]
masked:   [salary,  0,  tenure,   0,   ...]  (age and region zeroed)
```

**What it teaches**: No single feature is essential — the model must learn redundant representations that can reconstruct meaning from partial information.

**Analogy**: Like randomly erasing rectangular patches from an image — the model can't rely on any one region.

**Design choice**: The mask is per-sample but shared across set elements. This means "if age is masked for this customer, it's masked for ALL their transactions." This forces the model to learn cross-feature relationships rather than just reconstructing the masked feature from other elements.

### SubsetSample — Element Dropout

```python
mana.mimic.SubsetSample(keep_fraction=0.8)
```

Randomly drops set elements by modifying the boolean mask. Each element independently survives with probability `keep_fraction`. At least one element always survives.

```
set: {tx₁, tx₂, tx₃, tx₄, tx₅}
after: {tx₁, tx₃, tx₅}  (tx₂ and tx₄ dropped)
```

**What it teaches**: The set-level representation shouldn't depend on any single element. The model must learn to capture the "gist" of the set from any sufficient subset.

**Analogy**: You should recognise a customer's profile whether you see 5 of their transactions or 50. The representation should be robust to which specific transactions you observe.

**Safety guarantee**: Always keeps at least 1 element per set. If all elements are randomly dropped, one valid element is restored at random. This prevents the model from seeing an empty set, which would produce a degenerate zero embedding.

## Composing Augmentations

Real augmentation pipelines stack multiple augmentations:

```python
augmentation = mimic.Compose([
    mimic.SubsetSample(keep_fraction=0.8),  # Drop 20% of elements
    mimic.FeatureNoise(std=0.1),            # Add noise to survivors
])
```

`Compose` applies augmentations sequentially, threading the mask through. If `SubsetSample` modifies the mask, `FeatureNoise` sees the updated mask (though noise doesn't use the mask — it adds noise everywhere).

**Order matters**: SubsetSample first, then noise/masking is the natural order. You want to select which elements survive, *then* perturb them.

## Curriculum Augmentation: Start Gentle, End Strong

**Concept**: If you apply aggressive augmentations from the start, the model sees views that are too different to relate. It's like asking a student to solve calculus before they know algebra. **Curriculum learning** starts with easy examples and gradually increases difficulty.

```python
base_aug = mimic.FeatureNoise(std=0.5)     # Full strength = 0.5
curriculum = mimic.CurriculumAugmentation(
    base_aug,
    start_strength=0.1,   # Begin at 10% of full strength (std=0.05)
    end_strength=1.0,     # End at 100% of full strength (std=0.5)
)
```

The trainer automatically calls `set_progress(epoch / total_epochs)` each epoch. The augmentation's parameters are linearly interpolated:

```
At epoch 0:     strength = 0.1 → std = 0.5 × 0.1 = 0.05
At epoch 25:    strength = 0.55 → std = 0.5 × 0.55 = 0.275
At epoch 50:    strength = 1.0 → std = 0.5 × 1.0 = 0.5
```

### What Gets Scaled

The curriculum wrapper detects and scales common augmentation parameters:

| Parameter | Augmentation | Scaling behaviour |
|-----------|-------------|-------------------|
| `std` | FeatureNoise | `std × strength` (less noise → more noise) |
| `p` | FeatureMask | `p × strength` (fewer masked → more masked) |
| `keep_fraction` | SubsetSample | `1 - (1-kf) × strength` (keep more → keep less) |

Note that `keep_fraction` is inverted: at strength=0, you keep everything (easy); at strength=1, you keep the original fraction (hard).

### When to Use Curriculum

- **Use it** when your augmentations are aggressive (high noise, low keep fraction, high mask probability). The model needs time to learn basic structure before dealing with heavy distortion.
- **Skip it** when your augmentations are mild. The overhead of ramping adds nothing if the full-strength augmentation is already easy enough.

## Augmentation Recommendations by Data Type

| Data Type | Recommended Pipeline |
|-----------|---------------------|
| **Financial transactions** | SubsetSample(0.8) + FeatureNoise(0.05) — transactions are fairly precise, light noise is enough |
| **Medical records** | FeatureMask(0.2) + FeatureNoise(0.1) — tests may be missing, values have measurement error |
| **Text features** (bag-of-words) | FeatureMask(0.15) + SubsetSample(0.7) — words can be missing, documents vary in length |
| **Sensor readings** | FeatureNoise(0.2) + SubsetSample(0.9) — high measurement noise, readings usually all present |

## The Train/Eval Switch

All augmentations respect PyTorch's train/eval mode:

```python
model.train()   # Augmentations are active
model.eval()    # Augmentations pass input through unchanged
```

This means:
- During training: each forward pass creates *different* augmented views
- During inference (`model.encode(x, mask)`): the model sees the original, unaugmented data

`MimicModel.encode()` and `MimicModel.embed()` automatically switch to eval mode and back, so you don't need to manage this manually.
