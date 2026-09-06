# Encoders

## Purpose

Turn raw column values into tensor-ready representations. Every encoder follows the same protocol: `fit()` → `transform()` → `inverse_transform()`, with serialisation via `save_encoder()` / `load_encoder()`.

## The Three Encoder Types

### LabelEncoder

Maps categorical strings to integer indices. Optionally normalises to [0, 1].

```python
from mana.encoders import LabelEncoder

le = LabelEncoder(name="merchant")
le.fit(["grocery", "gas", "restaurant", "gas"])
le.transform(["gas", "grocery"])   # [0, 1]
le.decode(0)                       # "gas"
```

Good for: categories with moderate cardinality. For high-cardinality IDs (millions of users), consider an embedding layer instead.

### NumericEncoder

Min-max normalisation to [0, 1]. Learns min/max from `fit()`.

```python
from mana.encoders import NumericEncoder

ne = NumericEncoder(name="price")
ne.fit([10.0, 50.0, 100.0])
ne.transform([30.0])  # [0.222]
```

Handles edge cases: if min == max (constant column), returns 0.0 instead of dividing by zero.

### TextEncoder

Sentence embeddings via `sentence-transformers/all-MiniLM-L6-v2`. Pretrained — `fit()` is a no-op. Returns L2-normalised, mean-pooled embeddings.

```python
from mana.encoders import TextEncoder

te = TextEncoder(name="description")
te.fit([])  # no-op
embeddings = te.transform(["blue running shoes", "red hiking boots"])
# embeddings.shape → (2, 384)
```

## Batch Operations

For DataFrames with many columns, use the factory + batch functions:

```python
from mana.encoders import create_encoders, fit_encoders, apply_encoders

schema = [
    {"name": "category", "type": "categorical", "key": "category"},
    {"name": "price", "type": "numeric", "key": "price"},
]

encoders = create_encoders(schema)
fitted = fit_encoders(schema, df, encoders)
encoded_df = apply_encoders(schema, df, fitted)
```

## Design Notes

- All encoders use joblib for serialisation (safe for cross-version compatibility).
- The `EncoderProtocol` is a Python Protocol (structural typing), not an ABC. Any object with the right methods works — you don't need to inherit.
- Encoders are intentionally simple. Complex feature engineering (feature crosses, binning, target encoding) belongs in your data pipeline, not in the encoder.
