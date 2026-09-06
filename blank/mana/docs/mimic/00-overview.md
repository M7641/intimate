# Mimic: Contrastive Learning for Set-Structured Data

## What Problem Does This Solve?

You have **sets of objects** — a customer has a set of transactions, a patient has a set of lab results, a document has a set of paragraphs. You want to turn each _set_ into a single fixed-length vector (an **embedding**) that captures its meaning, so that similar sets end up close together and different sets end up far apart.

The catch: sets have **no fixed order** and **no fixed size**. A customer with 3 transactions and a customer with 300 transactions both need to produce a vector of the same length. And the result shouldn't change if you shuffle the order of the transactions.

Mimic solves this with **contrastive learning** — a self-supervised technique that learns embeddings by comparing augmented views of the same data.

## The Core Idea: Contrastive Learning

The fundamental insight is beautifully simple:

1. Take a data sample (a set of objects)
2. Create **two augmented views** of it (e.g. drop some elements, add noise)
3. Encode both views into embeddings
4. **Pull** the two views of the same sample together
5. **Push** views of different samples apart

That's it. No labels required. The model learns useful representations just by understanding "these two views came from the same thing."

```
  Set A ──┬── augment ──→ view A₁ ──→ encoder ──→ z_A1 ──┐
          │                                                ├── pull together
          └── augment ──→ view A₂ ──→ encoder ──→ z_A2 ──┘

  Set B ──┬── augment ──→ view B₁ ──→ encoder ──→ z_B1 ──┐
          │                                                ├── push apart from A
          └── augment ──→ view B₂ ──→ encoder ──→ z_B2 ──┘
```

## Architecture Pipeline

Every forward pass flows through this pipeline:

```
Raw Set (B, N, D)
    │
    ├── Augmentation ──→ View 1 (B, N, D)  ──→ Encoder ──→ Projector ──→ z₁
    │
    └── Augmentation ──→ View 2 (B, N, D)  ──→ Encoder ──→ Projector ──→ z₂
                                                                          │
                                                              Loss(z₁, z₂)
```

**After training**, you throw away the projector and use the encoder outputs as your embeddings. The projector exists only to give the loss function a space to work in — the encoder's representation space is richer and more useful for downstream tasks (this is a key finding from SimCLR).

## Component Map

| Component           | What It Does                                       | Options                                                                               |
| ------------------- | -------------------------------------------------- | ------------------------------------------------------------------------------------- |
| **Element Encoder** | Transforms individual elements within a set        | `MLPElementEncoder`, `TabularElementEncoder`                                          |
| **Aggregator**      | Pools variable-length elements into a fixed vector | `MeanAggregator`                                                                      |
| **Set Encoder**     | Full set → vector pipeline                         | `DeepSetsEncoder`, `SetTransformerEncoder`                                            |
| **Augmentation**    | Creates different views of the same set            | `FeatureNoise`, `FeatureMask`, `SubsetSample`, `Compose`, `CurriculumAugmentation`    |
| **Projector**       | Maps embeddings to contrastive space               | `MLPProjector`                                                                        |
| **Loss**            | Drives the "pull together / push apart" objective  | `NTXentLoss`, `SupConLoss`                                                            |
| **Regularizers**    | Encourage spread, dimensional health, decorrelation | `uniformity_loss`, `variance_loss`, `covariance_loss`                                 |
| **Trainer**         | Training loop with scheduling and curriculum       | `ContrastiveTrainer`                                                                  |
| **Evaluation**      | Measures embedding quality                         | `knn_accuracy`, `linear_probe`, `alignment`, `uniformity`, `silhouette`               |
