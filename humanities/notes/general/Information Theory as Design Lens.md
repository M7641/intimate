This is not about a technology. It's about a way of seeing.

DV2 data modelling already uses an autoencoder metaphor: compress raw data into a DV2 format (hubs, links, satellites), then expand into application-specific formats (Kimball stars, API responses, dashboards). That metaphor is more precise than it seems. What's happening is literally compression and decompression in the information-theoretic sense — and understanding the theory gives you tools to reason about data pipelines, ML systems, APIs, and product design with unusual clarity.

## Shannon's Core Insight

Claude Shannon's 1948 paper, "A Mathematical Theory of Communication," asks a deceptively simple question: what is information? His answer: information is the reduction of uncertainty.

Before you receive a message, you're uncertain about its content. After you receive it, you're less uncertain. The amount of uncertainty reduced — measured in bits — is the information content of the message.

A message that tells you something you already knew carries zero information. A message that tells you something completely unexpected carries maximum information. This isn't philosophical — it's mathematical:

```
Information(event) = -log₂(probability(event))
```

A fair coin flip (p = 0.5) carries 1 bit of information. A dice roll (p = 1/6) carries ~2.58 bits. An event that always happens (p = 1.0) carries 0 bits. An event that never happens (p → 0) carries infinite bits.

**Entropy** is the average information content of a source — the expected number of bits per message:

```
H(X) = -Σ p(x) × log₂(p(x))
```

A source with high entropy is unpredictable (lots of information per message). A source with low entropy is predictable (little information per message, lots of redundancy). This distinction is the foundation of everything that follows.

## Compression: Removing Redundancy Without Losing Meaning

Compression is the act of encoding a message using fewer bits than the raw representation, by exploiting redundancy. Shannon proved a fundamental theorem: you can compress a source down to its entropy (the average information content) but no further. Below that limit, you lose information.

There are two kinds of compression:

**Lossless compression** preserves every bit of the original. ZIP, gzip, LZ4, ZSTD — these exploit statistical redundancy (repeated patterns) to reduce size. The decompressed output is identical to the original. Redshift column encodings are a concrete example: AZ64 for timestamps, ZSTD for high-cardinality strings, BYTEDICT for low-cardinality fields. Each encoding exploits a different kind of redundancy in the data.

**Lossy compression** discards information that's deemed unimportant. JPEG discards high-frequency visual details that humans can't perceive. MP3 discards audio frequencies outside human hearing. The decompressed output is different from the original, but the *meaningful* content is preserved.

Here's where it gets interesting for system design: most data pipelines perform lossy compression without calling it that.

## DV2 as Information-Theoretic Compression

The DV2 pipeline takes raw data (high entropy — every field, every row, every timestamp, every artefact of the source system) and compresses it into three structures:

**Hubs** are extreme compression. From a full customer record (name, address, phone, email, history, preferences, scores), the hub retains only the business key. Everything else is discarded. In information theory terms: the hub preserves only the *identity* information — the minimum bits needed to distinguish one entity from another. Everything else is redundant relative to identity.

**Links** preserve only *relationship* information — which entities connect to which. A link between a customer hub and an order hub says "this customer placed this order." It discards the details of both the customer and the order. In information theory: the link encodes the mutual information between two entities — the information that knowing one gives you about the other.

**Satellites** are the residual — the information that hubs and links discarded. Attributes, descriptions, measurements, timestamps. They're keyed to hubs or links, providing the detail when needed. In compression terms: the satellite is the "detail layer" that, combined with the hub/link skeleton, reconstructs the full picture.

This decomposition is exactly how modern image compression works. JPEG separates an image into a low-frequency base (the overall shape — analogous to hubs and links) and high-frequency detail (texture and edges — analogous to satellites). The base is compact and stable; the detail is large and variable.

The insight: DV2 isn't just a data modelling methodology. It's a compression scheme that separates identity, relationships, and attributes because they have different entropy profiles and different rates of change. Hubs change almost never (business keys are stable). Links change rarely (relationships are relatively stable). Satellites change frequently (attributes update constantly). Compressing data along these change-frequency boundaries is information-theoretically optimal — you store the stable skeleton once and only update the volatile details.

## The Rate-Distortion Trade-off

Every lossy compression involves a trade-off between *rate* (how compressed) and *distortion* (how much information is lost). Shannon formalised this as the rate-distortion function: for a given level of acceptable distortion, what is the minimum number of bits needed?

This trade-off appears everywhere in system design:

**API responses.** A full customer record might be 50 fields. An API response for a dashboard needs 8. The API is a lossy compression of the database — it preserves only the fields relevant to the consumer. The rate-distortion question: which 8 fields preserve the most information for the dashboard's purpose? Include too few and the dashboard is useless (high distortion). Include too many and the API is slow and the consumer is overwhelmed (high rate).

**Feature engineering for ML.** Raw data has thousands of potential features. The model needs a subset. Feature selection is lossy compression — which features preserve the most information about the prediction target while minimising redundancy between features? Mutual information (a direct Shannon concept) is used to measure this: how many bits of information does feature X provide about target Y?

**Contrastive learning connects directly.** Contrastive learning trains an encoder to produce a compressed representation (embedding) that preserves similarity information while discarding irrelevant variation. The loss function (contrastive loss, triplet loss, InfoNCE) is literally an information-theoretic objective: maximise the mutual information between the embedding and the identity of the object, minimise the mutual information between the embedding and the noise.

**Database indexing.** An index is a lossy compression of a table — it preserves the sort order and key values while discarding the full row data. The rate-distortion trade-off: a covering index (includes all needed columns) has higher rate but zero distortion for its intended query. A minimal index has lower rate but requires a table lookup (additional distortion in terms of latency).

**Boundary observation.** Observing only the edges (data in, data out) while ignoring internals is compression. You're discarding the high-entropy internal state (which changes constantly and in unpredictable ways) and retaining only the low-entropy boundary signals (which conform to contracts). The rate-distortion question: are the boundary signals sufficient to detect problems? If yes, the compression is efficient. If internal failures aren't visible at the boundary, you're over-compressing — losing information that matters.

## Entropy as a Diagnostic Tool

Entropy can be measured on real data, and the measurements tell you things:

**Column entropy reveals compression opportunities.** A column with 5 unique values across 10 million rows has very low entropy — it's almost entirely redundant. BYTEDICT encoding exploits this. A column with 9.5 million unique values across 10 million rows has high entropy — it's nearly incompressible. Understanding the entropy profile of your data before choosing compression encodings turns the encoding decision from intuition into calculation.

**Schema entropy reveals design problems.** If a customer's schema has 200 columns but the entropy analysis shows that 150 of them are redundant with the other 50 (high mutual information between columns), the schema is over-specified. The 150 redundant columns add storage cost, query complexity, and maintenance burden without adding information. This is a formal argument for schema simplification that goes beyond "it feels like too many columns."

**Pipeline entropy reveals data quality.** If a column that should have low entropy (e.g., country codes — a fixed set of values) suddenly shows high entropy, something is wrong. New values are appearing that shouldn't exist. Monitoring entropy over time is a data quality check that catches anomalies the schema can't express: "This column has the right type but the wrong distribution."

**Feature entropy reveals model inputs.** A feature with zero entropy (constant value) provides no information to a model. A feature with maximum entropy (uniformly random) provides no *useful* information. The best features have intermediate entropy — enough variation to be informative, enough structure to be predictable. Mutual information between feature and target quantifies exactly how useful a feature is, in bits.

## Kolmogorov Complexity and the Simplicity Principle

There's a deeper thread. Kolmogorov complexity measures the information content of an individual object (not a probabilistic source): the length of the shortest program that produces it. A string of random characters has high Kolmogorov complexity — the shortest program that produces it is essentially the string itself. A string of repeated characters has low Kolmogorov complexity — the program "print 'a' 1000 times" is much shorter than the string.

This formalises the intuition behind a familiar line: "Any intelligent fool can make things bigger, more complex, and more violent. It takes a touch of genius — and a lot of courage — to move in the opposite direction."

The line is about Kolmogorov complexity. A system with high Kolmogorov complexity requires a long description — many special cases, many exceptions, many conditional branches. A system with low Kolmogorov complexity has a short description — a few principles that generate the full behaviour. The goal of good design is to minimise the Kolmogorov complexity of the system: find the shortest description (the simplest model, the fewest principles) that produces the desired behaviour.

DV2's three-concept model (hubs, links, satellites) is low Kolmogorov complexity. Three concepts generate an arbitrarily large data architecture. Each customer's bespoke pipeline is high Kolmogorov complexity — every special case needs its own description. The product transition is, formally, a compression of the solution space: finding the low-complexity model that covers the high-complexity cases.

## The Minimum Description Length Principle

MDL bridges information theory and machine learning directly. It says: the best model is the one that minimises the total description length — the length of the model itself plus the length of the data encoded using the model.

A very simple model (short description) may encode the data poorly (long residual). A very complex model (long description) encodes the data perfectly but the model itself is expensive. The optimal model balances these — it captures the regularities in the data (compressing them into model parameters) without overfitting to noise (which would make the model longer without improving compression).

This is Occam's Razor formalised in bits. And it applies directly to system design:

- A product with too few configuration options (simple model) forces customers to work around limitations (large residual / high distortion)
- A product with too many configuration options (complex model) is expensive to maintain and hard to understand, even if it fits every customer perfectly
- The optimal product captures the regularities across customers (the shared patterns) in its core model and handles the residual (customer-specific variation) through minimal, well-designed extension points

The bespoke-to-product journey is MDL optimisation. You're searching for the model (product) that minimises total description length: the product's complexity plus the remaining per-customer customisation.

## Practical Applications

**Data validation as entropy monitoring.** Compute column-level entropy for incoming data batches. Alert when entropy deviates significantly from historical baseline. This catches data quality issues (new categories appearing, distributions shifting, null rates changing) without writing specific validation rules for each case.

**Feature selection as mutual information maximisation.** For the ML pipeline, rank features by mutual information with the target variable. Select the top-k features that maximise total information while minimising redundancy between features (maximum relevance, minimum redundancy — the mRMR algorithm). This is more principled than correlation-based feature selection because it captures nonlinear relationships.

**API design as rate-distortion optimisation.** For each API endpoint, ask: what is the minimum set of fields that satisfies the consumer's information need? Every additional field increases rate (payload size, serialisation cost, coupling surface) without reducing distortion (the consumer ignores it). Design APIs at the rate-distortion frontier.

**Schema design as entropy decomposition.** Group columns by entropy profile (stable low-entropy columns together, volatile high-entropy columns together). This naturally produces the hub/satellite split: hubs contain low-entropy identifiers, satellites contain high-entropy attributes. The decomposition is now a calculation, not a judgment call.

**Compression as a test of understanding.** If you can't compress a system's description (explain it more concisely than the code itself), you don't fully understand its regularities. The ability to compress is evidence of understanding. This is why concise writing is a discipline worth practising — writing concisely forces you to identify the essential structure, which is literally what information theory measures.

## Reading

- Claude Shannon, "A Mathematical Theory of Communication" (1948) — the original paper. 79 pages. Read it. It's clearer than most textbook treatments because Shannon was an exceptionally good writer.
- David MacKay, *Information Theory, Inference, and Learning Algorithms* (2003) — the best textbook, available free online. Covers information theory, compression, error correction, and the connection to machine learning. The Bayesian inference chapters are particularly relevant.
- Jorma Rissanen, "Modeling by Shortest Data Description" (1978) — the MDL principle paper. Short, foundational.
- Gregory Chaitin, "Meta Math!" (2005) — a readable introduction to Kolmogorov complexity and algorithmic information theory, by one of its co-discoverers.
- Peter Grünwald, *The Minimum Description Length Principle* (2007) — the comprehensive treatment of MDL and its applications to model selection.
- Cover and Thomas, *Elements of Information Theory* (2006) — the standard graduate textbook. Dense but complete. Use as reference, not as first reading.

## The Meta-Point

Information theory provides a formal vocabulary for decisions that are otherwise made by feel. "This schema is too complex" becomes "this schema has high Kolmogorov complexity." "This feature is useful" becomes "this feature has high mutual information with the target." "This API returns too much data" becomes "this API operates above the rate-distortion frontier." "This product is too configurable" becomes "the model description length exceeds the compression it provides."

The vocabulary doesn't make the decisions for you. But it makes the decisions *arguable* — you can measure, compare, and justify, rather than relying on aesthetics alone. And for anyone with a consistent drive toward clarity, precision, and principled simplification, that vocabulary might be the most useful tool available.
