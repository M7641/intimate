# Tabular Encoders: Problem Statement & Architecture Survey

## The problem

Tabular data is the format where deep learning has historically _lost_ to gradient-boosted decision trees, and understanding why is the key to choosing an encoder. The naive intuition — "a neural net is a universal approximator, so it should match a tree at worst" — is wrong because it conflates **representational capacity** (what a model class _can_ express) with **what gets learned from finite data under a given optimizer**. Universal approximation guarantees a weight configuration exists; it says nothing about whether SGD reaches it from limited noisy data, or whether it generalises. A more expressive class with the wrong inductive bias generalises _worse_ at fixed data, not better.

Tabular data has specific structure that determines which inductive bias wins:

- **Irregular, non-smooth targets.** Dependencies are often sharp and threshold-like ("if income > X and tenure < Y"). Trees fit axis-aligned piecewise-constant functions — exactly this shape. MLPs have a spectral bias toward smooth, low-frequency functions and resist sharp boundaries.
- **Not rotationally invariant.** Each column has individual meaning; informative directions are axis-aligned. MLPs are approximately invariant to input rotations, so they waste capacity rediscovering that the canonical axes are special. Trees are axis-aligned by construction.
- **Many uninformative features.** Trees do greedy feature selection at every split, ignoring noise. MLPs entangle all features in the first layer and must learn to suppress junk.
- **Heterogeneous, messy features.** Different scales, skew, heavy tails, categoricals, missing values. Trees are invariant to per-feature monotonic transforms (only rank matters); NN optimisation is brittle on this without careful preprocessing.
- **Small-to-medium data regime.** A strong, correct prior beats a weak one when data is scarce. NNs' representation-learning edge needs scale (or pretraining) to amortise their weak prior.

**Implication for an encoder.** The gap was never about information content or capacity — it was about _prior_. Deep models become competitive when the missing tabular inductive bias is supplied: by construction (axis-aligned attention, numerical embeddings), by retrieval (kNN-style attention), or by synthetic pretraining (TabPFN). The central design axis for a custom encoder is how it treats feature identity:

- **Feature-identity-preserving** (per-feature tokens/embeddings) — exploits stable column meaning; best for fixed schemas.
- **Permutation-invariant over features** (row-as-set) — discards column identity, which is a _handicap_ on fixed schemas but the _enabling property_ for schema-agnostic / cross-table embedding, where feature sets vary.

A Set Transformer used as a tabular encoder sits in the second camp: it only makes sense in the cross-schema / concept-agnostic framing, and should be benchmarked against cross-table models (CARTE/TabPFN), not against FT-Transformer on a single fixed table.

---

## Architecture survey

### Gradient-boosted decision trees — LightGBM / XGBoost / CatBoost

Stagewise additive ensemble of shallow trees fitting residuals (functional gradient descent). LightGBM adds histogram binning, leaf-wise growth, GOSS, EFB, and native categorical splits.

- **Pros:** State-of-the-art accuracy on most fixed-schema tabular tasks; strong out-of-the-box with minimal tuning; invariant to feature scaling/skew/outliers; native handling of missing values and (CatBoost/LightGBM) categoricals; fast; self-regularising; interpretable via feature importance/SHAP.
- **Cons:** Not an encoder — produces predictions, not reusable embeddings (leaf indices are a weak substitute); no cross-table transfer; poor with very high-cardinality or unstructured features; struggles to leverage large data or pretraining; weak at modelling raw high-dimensional interactions a deep net could learn.

### MLP + entity embeddings

Plain feed-forward net with learned embeddings for categoricals, concatenated with normalised numerics.

- **Pros:** Simple, fast, produces an embedding; easy to extend/multitask; reasonable with heavy preprocessing.
- **Cons:** Weak inductive bias for tabular (smoothness, rotational quasi-invariance); sensitive to scaling and noise features; usually loses to GBDTs without tricks; no feature-identity attention.

### FT-Transformer (Feature Tokenizer + Transformer)

Tokenises every feature (numeric via learned projection, categorical via embedding), prepends a CLS token, runs self-attention.

- **Pros:** Strongest "vanilla" deep baseline; keeps explicit feature identity; CLS token is a clean reusable embedding; handles feature interactions natively; pairs well with numerical-feature embeddings (piecewise-linear/periodic).
- **Cons:** Fixed schema (per-feature tokens are dataset-specific); still often matched/beaten by GBDTs on small data; O(features²) attention; more tuning than a tree.

### TabTransformer

Self-attention over categorical embeddings only; continuous features bypass attention and concatenate at the head.

- **Pros:** Good when categoricals dominate; contextual categorical embeddings transfer reasonably.
- **Cons:** Continuous features get no attention (a real limitation); largely superseded by FT-Transformer.

### AutoInt

Multi-head self-attention to learn explicit bounded-degree feature interactions; CTR/recommendation heritage.

- **Pros:** Explicit, somewhat interpretable feature crossing; strong on high-cardinality sparse categorical (ad/rec) data.
- **Cons:** Tuned for sparse CTR settings; less general; modest gains on dense heterogeneous tables.

### SAINT

Adds **inter-sample attention** (attending across rows, not just features) plus contrastive/self-supervised pretraining.

- **Pros:** Row-context attention captures cross-sample structure a per-row encoder misses; pretraining helps in low-label regimes; competitive accuracy.
- **Cons:** Quadratic in both rows and features (memory-heavy); more complex training; benefit varies by dataset.

### Set Transformer (row-as-set)

ISAB for inter-feature interactions, PMA pooling to a fixed-width embedding; permutation-invariant over features.

- **Pros:** Naturally schema-flexible (variable/arbitrary feature sets); ISAB gives O(features × inducing points) instead of quadratic; clean fixed-width embedding; ideal backbone for a concept-agnostic embedding engine.
- **Cons:** Permutation invariance discards informative column identity — a handicap on fixed schemas; no column-name semantics unless added; no inter-sample attention by default; will lose to FT-Transformer/TabR on single fixed tables.

### TabNet

Sequential attention with sparse feature-selection masks at each decision step.

- **Pros:** Built-in interpretable feature selection; instance-wise feature attention; decent on larger data.
- **Cons:** Finicky to train; inconsistent vs GBDTs across benchmarks; sensitive to hyperparameters.

### NODE (Neural Oblivious Decision Ensembles)

Differentiable oblivious decision trees stacked into a deep ensemble.

- **Pros:** Tree-like inductive bias in a differentiable, end-to-end model; competitive on some benchmarks.
- **Cons:** Heavier and slower than GBDTs for similar/worse accuracy; less adopted; limited transfer story.

### DCNv2 (Deep & Cross Network)

Explicit bounded-degree feature crossing layers alongside a deep tower.

- **Pros:** Efficient explicit interactions; strong in recommendation/CTR; scalable.
- **Cons:** Domain-specialised; modest on general dense tabular tasks.

### TabR / ModernNCA (retrieval-augmented)

Learned kNN-style attention over the training set at inference (parametric + non-parametric hybrid).

- **Pros:** Among the most reliable deep approaches at closing the gap to GBDTs on the standard benchmark suites; non-parametric component adapts to local structure; produces useful representations.
- **Cons:** Inference needs access to a reference set (latency/memory); retrieval index management; more moving parts.

### TabPFN family (in-context foundation models)

Prior-data-fitted transformer pretrained on millions of synthetic structural-causal-model datasets; does training-free Bayesian posterior-predictive inference in one forward pass. Uses two-way (row + column) attention, so it is permutation-invariant over both features and samples. v2.5 (late 2025) reports a 100% win rate vs default XGBoost on classification up to ~10k rows / 500 features, and ~87% up to 100k rows / 2k features, per the authors. Scaling variants: TabICL (column-then-row, ~500k rows), TabFlex (linear attention, millions of rows), Mitra (curated synthetic-prior mixtures), TabPFN-Wide (>50k features, biomedical).

- **Pros:** Often beats GBDTs in its size band with zero per-dataset training; calibrated uncertainty; the prior supplies the missing tabular inductive bias; usable as a frozen featurizer; fast at small scale.
- **Cons:** Bounded by pretraining regime (rows/features/classes) unless using a scaling variant; large model/compute for big data; in-context cost grows with the reference set; benchmark caveats (some leaderboards involve the authors' lab); distillation needed for low-latency production.

### Cross-table / schema-agnostic — CARTE / TARTE, TransTab, XTab

Pretrained over many heterogeneous tables; CARTE/TARTE fuse column-name semantics (string-encoded) with cell values via a graph-attention transformer.

- **Pros:** Genuine cross-schema transfer and embeddings across tables with different feature sets; column-name semantics give strong few-shot/small-data performance, reportedly beating trees on small datasets; the right comparison class for a concept-agnostic encoder.
- **Cons:** Fine-tuning and high compute cost; relies on meaningful column names; younger ecosystem; integration overhead.

### Self-supervised pretraining recipes — SCARF / VIME / SubTab

Not architectures but pretraining objectives (corruption-contrastive, masked reconstruction, sub-setting) that bolt onto any encoder above.

- **Pros:** Improve low-label performance; orthogonal and composable; natural pairing with a set/contrastive encoder.
- **Cons:** Gains are dataset-dependent; add training complexity; no inductive-bias fix on their own.

---

## Choosing

- **Single fixed-schema table, accuracy is the goal:** start with LightGBM/CatBoost as the bar; if you want a deep encoder, use FT-Transformer or TabR with numerical-feature embeddings. A Set Transformer is the wrong tool here.
- **Small data, want a strong baseline fast:** TabPFN-2.5 (as predictor or frozen featurizer), benchmarked against a tuned GBDT.
- **Schema-agnostic / cross-table embedding engine (the Set Transformer use case):** compare against CARTE/TARTE and the TabPFN lineage; the highest-leverage additions to a set encoder are (1) column-name conditioning via a frozen text encoder over headers, and (2) an inter-sample attention path (ISAB inducing points over the batch). Those two changes are where most of the gap to current cross-table SOTA lives.
