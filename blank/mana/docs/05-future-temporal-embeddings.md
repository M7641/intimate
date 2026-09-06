# Future: Temporal Set Embeddings

## The Problem

Mimic today treats each entity as a **static set** — a bag of elements with no time axis. But real entities evolve:

- A customer's transactions change quarter to quarter
- A patient's lab results drift as treatment progresses
- A product's review set shifts after a redesign

We can snapshot an entity at a point in time and embed that snapshot, but we lose the trajectory. Two customers with identical current transaction sets but opposite trends (one ramping up, one winding down) get the same embedding.

## The Question

**How do we embed sets that have variance over time?**

This sits at the intersection of two well-studied problems — set encoding (order-invariant within a set) and sequence modelling (order-dependent across time) — but the combination is underexplored.

### What We Want

```
Entity at t=1:  {a, b, c}
Entity at t=2:  {a, b, d, e}
Entity at t=3:  {b, e, f}
                    ↓
        single embedding that captures
        both set structure AND temporal dynamics
```

The embedding should be:
- **Permutation-invariant** within each timestep (it's still a set)
- **Order-sensitive** across timesteps (temporal direction matters)
- **Variable-length** in both dimensions (set sizes and number of timesteps can vary)

## Possible Approaches

### 1. Encode-then-sequence

The simplest composition: use mimic to embed each timestep's set independently, then feed the sequence of embeddings into a temporal model.

```
{set_t1} → mimic → h1 ─┐
{set_t2} → mimic → h2 ──┼── Transformer/LSTM → temporal embedding
{set_t3} → mimic → h3 ─┘
```

Pros: clean separation, reuse existing mimic. Cons: the set encoder can't attend across time — if an element appearing in t1 and t3 but not t2 is meaningful, the per-timestep encoder can't see that.

### 2. Flatten with positional encoding

Treat all elements across all timesteps as one big set, but add a temporal positional encoding to each element so the model can distinguish "transaction from last week" from "transaction from last year".

```
[elem_1_t1, elem_2_t1, ..., elem_1_t2, elem_2_t2, ...] + time_encoding
                    ↓
            Set Transformer → embedding
```

Pros: the attention mechanism can directly relate elements across time. Cons: quadratic attention cost over all elements × all timesteps, loses the explicit set-per-timestep structure.

### 3. Hierarchical attention

A two-level transformer: inner attention within each timestep (set-level, permutation-invariant), outer attention across timesteps (sequence-level, order-sensitive).

```
Timestep 1: {a, b, c} → Set Transformer → h1
Timestep 2: {a, b, d} → Set Transformer → h2  → Temporal Transformer → embedding
Timestep 3: {b, e, f} → Set Transformer → h3
```

This is conceptually closest to what we want. The inner layer handles permutation invariance, the outer handles temporal order. Shared weights across timesteps or not is a design choice.

### 4. Contrastive objective for temporal data

The training signal needs rethinking too. Current mimic augmentations (subset sample, feature noise) work within a single set. For temporal data, we could:

- **Temporal crop**: two views = two overlapping windows of the entity's history
- **Temporal jitter**: slightly shift timestamps
- **Cross-time positive pairs**: the same entity at nearby timesteps should embed closer than different entities at the same timestep

This connects to video contrastive learning (CVRL, VideoMoCo) where frames play the role of timesteps.

## Open Questions

1. **Granularity of time**: discrete timesteps (weekly snapshots) vs continuous time (exact timestamps with positional encoding)? Discrete is simpler but loses within-period ordering.

2. **Shared vs separate encoders**: should the per-timestep set encoder share weights across time? Shared = fewer params, assumes stationarity. Separate = can model distributional shift, but needs more data.

3. **How far back matters?** Attention over 100+ timesteps is expensive. Windowed attention? Exponential decay? Summarise distant history into a single vector?

4. **Relationship to the forecast adapter**: the LSTM forecast module already models temporal sequences. Could a temporal set encoder replace or augment it? The LSTM operates on scalar features per timestep; a temporal set encoder would operate on *sets* of features per timestep — strictly more general.

5. **Evaluation**: what downstream task validates temporal embeddings? Predicting next-period behaviour (churn, purchase category)? Detecting changepoints? The evaluation strategy drives the architecture choice.

## References to Explore

- Set Transformer (Lee et al., 2019) — the set encoding backbone we already use
- Temporal Fusion Transformers (Lim et al., 2021) — attention over time with static/dynamic feature separation
- CVRL (Qian et al., 2021) — contrastive learning on video (temporal crops as augmentation)
- Perceiver (Jaegle et al., 2021) — handles arbitrary input structures with cross-attention to a latent array, could unify set + time dimensions
