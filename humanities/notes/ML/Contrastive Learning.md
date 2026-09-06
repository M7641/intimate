
The core idea is to create **multiple views** of the same object using only its own descriptive data, then train an encoder to pull those views together while pushing apart views from different objects.

## Key Approaches

### 1. Augmentation-Based Pairs (SimCLR-style)

For a data object with fields/features, create positive pairs by:

- **Feature dropout** — randomly mask subsets of attributes
- **Field perturbation** — add noise, synonym replacement, or paraphrasing to text fields
- **Subsampling** — take different random subsets of the object's properties

The same object augmented two different ways forms a positive pair; different objects form negatives.

### 2. Multi-View from Structure (Split-and-Agree)

If your object has natural sub-structures (e.g. a product with title, description, specs, category), split them into complementary views:

```
View A: [title + category]  ←→  View B: [description + specs]
```

Train two encoders (or a shared one) so that views from the same object are close in embedding space. This is essentially **cross-view prediction** — each partial description should map to the same region.

### 3. Corruption-Based (Barlow Twins / VICReg style)

- Take the full object description, create two corrupted copies (dropout, shuffle field order, token masking)
- Optimise for invariance between the two embeddings while decorrelating embedding dimensions (avoiding collapse)

## Practical Setup

```python
# Pseudocode
def contrastive_step(batch_of_objects, encoder, temperature=0.07):
    # Create two augmented views of each object
    view_a = augment(batch_of_objects)  # e.g. mask 30% of fields
    view_b = augment(batch_of_objects)  # different random mask

    z_a = encoder(view_a)  # (B, D)
    z_b = encoder(view_b)  # (B, D)

    # NT-Xent / InfoNCE loss
    # Positives: (z_a[i], z_b[i])
    # Negatives: all other items in batch
    sim = cosine_similarity_matrix(z_a, z_b) / temperature
    labels = torch.arange(len(batch_of_objects))
    loss = cross_entropy(sim, labels)
    return loss
```

## Augmentation Strategies by Data Type

|Data type|Augmentation ideas|
|---|---|
|**Tabular**|Feature masking, Gaussian noise on numerics, shuffle categorical encoding|
|**Text descriptions**|Token masking, sentence dropout, back-translation, span corruption|
|**Structured (JSON/graphs)**|Subgraph sampling, field dropout, key reordering|
|**Multi-field**|Cross-field splits (natural multi-view)|

## Avoiding Collapse

The main risk with self-only contrastive learning is **representation collapse** (everything maps to the same point). Mitigations:

- **Large batch sizes** for diverse negatives
- **Hard negative mining** — objects that are similar but distinct
- **Non-contrastive regularisation** — variance/covariance constraints (VICReg), or stop-gradient (BYOL/SimSiam) which can even eliminate the need for explicit negatives
- **Asymmetric architectures** — predictor head on one branch only

## When This Works Well

This approach is strongest when objects have **rich, redundant self-descriptions** — meaning there's enough information in each object that partial views are still informative. If objects are described by only 2-3 sparse fields, you may need to supplement with relational signals (co-occurrence, user interactions) or use a generative objective instead (masked autoencoding).