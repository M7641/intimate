# Adjacent Work & When to Reach for This Stack

A companion to [`genisis.md`](./genisis.md). Where `genisis.md` explains _what_ Arrow / Parquet / DataFusion are and _why_ the serialisation-free columnar substrate matters, this document answers two practical questions:

1. **When** does a company that builds data-processing applications to create and train ML artefacts actually reach for this "data fusion" stack — and when does it not?
2. **What other work** is adjacent to this pilot — the threads worth pulling next, ordered by how directly they extend what already exists in `examples/`.

The pilot today is six examples (`01`–`06`) and an empty `lib.rs`. It proves the mechanics: write Parquet, read it back, run compute kernels, query with SQL, exchange via Arrow IPC, do window-function analytics. Everything below is about turning that proof into leverage.

---

## Part 1 — When to use this stack for an ML-artefact company

Cost centres are:

- **Feature pipelines** — the same logic often has to run twice: in batch (training) and online (serving). Divergence between the two is _training/serving skew_, a leading cause of silent model degradation.
- **Dataset materialisation** — turning raw data into the exact training set, reproducibly, cheaply, at scale.
- **Data movement** — between Python (where models are trained) and a production runtime (where they serve), and between local compute and a warehouse.
- **Validation** — proving the data is correct _before_ it poisons a model.

This stack pays off precisely where these cost centres bite.

### Decision signals — reach for it when…

| Signal                                                                                                           | Why this stack helps                                                                                                                                                                                                 |
| ---------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **You move tabular data between Python and a non-Python runtime** (Rust/Go/C++ service, or a different process). | Arrow is the zero-copy bridge. The bytes Python writes are the bytes the serving runtime reads — no JSON/CSV/protobuf round-trip, no type drift. Directly attacks training/serving skew.                             |
| **Training datasets are larger than memory but you only need slices.**                                           | Parquet predicate + projection pushdown (and page/row-group statistics) lets you read only the columns and rows you need. Sampling and filtering a 100 GB dataset stops requiring a 100 GB read.                     |
| **You need feature computation that is "too fresh for batch, too complex for a key-value lookup."**              | DataFusion runs SQL (joins, window functions, aggregations — see `examples/06_datafusion_analytics.rs`) in-process over the last N hours of events, returning Arrow arrays that feed inference with zero conversion. |
| **You want reproducible training sets.**                                                                         | Parquet files (and table formats layered on top — see Part 2) give immutable, content-addressable dataset snapshots. "Train on the data as of timestamp T" becomes tractable.                                        |
| **Analytical queries are currently round-tripping to a warehouse and you feel the latency/cost.**                | An embedded DataFusion engine answers sub-warehouse-scale queries in milliseconds, in-process, with no compute bill (see `examples/04_datafusion_sql.rs`).                                                           |
| **Validation is procedural and brittle.**                                                                        | Schema, uniqueness, and referential checks express naturally as SQL run against incoming data before it lands. Validation becomes declarative.                                                                       |
| **You serve analytical results or features to customers over a network.**                                        | Arrow Flight / Flight SQL sends typed columnar batches instead of parsed JSON — the consumer gets ready-to-use arrays.                                                                                               |

### Mapped to the ML lifecycle

```
INGEST ─────► VALIDATE ─────► FEATURE BUILD ─────► TRAIN ─────► SERVE
   │             │                  │                │            │
 Arrow IPC /   DataFusion       DataFusion +      Parquet      DataFusion
 object_store   SQL checks      compute kernels   snapshots    online features
 (ex 05)        (ex 04/06)      (ex 03/06)        (ex 01)      (ex 04/06)
                                      │                              │
                                      └──── same Arrow schema both sides ────┘
                                            (kills training/serving skew)
```

The single most defensible reason an ML-artefact company adopts this: **one schema and one memory layout from feature build through serving.** Everything else (latency, cost, reproducibility) is a bonus on top of skew elimination.

### When _not_ to reach for it

- **Your data fits comfortably in pandas/Polars and never leaves Python.** Then PyArrow under the hood already gives you most of the benefit; you don't need DataFusion or a Rust service. Don't build a bridge you won't cross.
- **Your artefacts are vision/NLP models over blobs (images, audio, raw text).** The bottleneck is GPU and blob I/O, not columnar tabular plumbing. Arrow still helps for _labels/metadata/manifests_, but it is not the main event. (Note: for _embeddings_ this flips — see Lance in Part 2.)
- **Single-node, single-language, modest scale, no serving-skew problem.** The stack's payoff is at the seams (process/language/storage boundaries). No seams, less payoff.
- **You have no appetite for Rust and the win is purely serving-side performance.** Consider DuckDB (embeddable, Python-first) before committing to a DataFusion service — same Arrow output, far lower adoption cost.

---

## Part 2 — Adjacent work worth doing

Ordered in three tiers: finishing the threads this pilot already started, then ML-specific extensions, then the broader platform plays. Each item notes roughly what it is, why it matters here, and a sense of effort.

### Tier 1 — Finish the threads `genisis.md` already opened

These are the explicit "next steps" the genesis doc gestures at but the pilot has not yet built.

1. **Custom `TableProvider` over S3 with row-level security.** _(medium)_
   The composability hook of DataFusion. Implement the `TableProvider` trait to read a customer's Parquet from object storage and inject per-customer row filtering _inside_ the provider, so isolation is structural, not a `WHERE` clause someone can forget. This is the doorway from "examples" to "product" — the genesis doc calls step 5 of Getting Started "the beginning of a product."

2. **The S3 shuttle, implemented.** _(small–medium)_
   `object_store` is already in the dependency tree via DataFusion. Wire `AmazonS3Builder` + `put` to upload the Parquet that `examples/01` and `examples/06` produce, then a `COPY INTO` on Snowflake/Redshift. Turns the warehouse-loading section of `genisis.md` from prose into a runnable path.

3. **The PyO3 bridge, as a real pilot.** _(medium)_
   A `lib.rs` (currently a 2-line stub) that exposes a Rust function computing features as Arrow `RecordBatch`es, handed to Python via PyO3 + `pyarrow` with zero copy. This is the concrete demonstration of the polyglot bridge and the strongest skew-killer. Until it exists, "zero-copy Rust↔Python" is a claim, not an artefact.

4. **An Arrow Flight / Flight SQL server.** _(medium–large)_
   A gRPC endpoint that returns Arrow batches instead of JSON. Validates the "serve features/analytics to consumers without a serialisation tax" thesis and makes the engine reachable from any Flight client (Python, JS, BI tools).

5. **Promote the examples into a small library.** _(small)_
   The 650 lines in `examples/` are pedagogical but not reusable. Extract the recurring patterns (schema builders, Parquet read/write helpers, a `SessionContext` factory) into `lib.rs` so downstream pilots `use fusion::…` instead of copy-pasting.

### Tier 2 — ML-specific extensions

Where the stack stops being generic plumbing and starts being an ML platform.

6. **Lance / LanceDB.** _(medium)_ — **the highest-leverage ML-specific addition.**
   Lance is a modern columnar format _designed for ML_: Arrow-native, with fast random access (critical for shuffled training reads, which Parquet does poorly), built-in versioning, and first-class vector columns + ANN indexing. For a company shipping embeddings and training data, Lance is the format Parquet wishes it were for ML workloads. A pilot: store `sensors`-style data plus an embedding column, do vector search, version a dataset, train against a specific version.

7. **Reproducible dataset versioning via Delta Lake (`delta-rs`) or Iceberg (`iceberg-rust`).** _(medium)_
   Both are Arrow/DataFusion-compatible table formats over Parquet with ACID logs and _time travel_. "Train on the data as of version N" → reproducible experiments and auditable artefacts. Directly supports the reproducibility decision-signal from Part 1.

8. **A feature-store-shaped pilot (or Feast integration).** _(medium–large)_
   Formalise the batch + online feature split: batch features materialised to Parquet/Lance, online features computed at request time via DataFusion, _both from one feature definition_. This is the skew story made operational. Feast can sit on top, or DataFusion can back a lightweight homegrown store.

9. **Validation layer as SQL (the "DV2" idea).** _(small–medium)_
   Turn schema conformance, uniqueness, and referential-integrity checks into a catalogue of DataFusion SQL queries run against incoming data before it reaches training. Pairs naturally with `examples/04`. Each failing check becomes a typed report, not a stack trace.

10. **GPU hand-off (cuDF / RAPIDS).** _(research)_
    cuDF consumes Arrow zero-copy. For an ML shop already on GPUs, Arrow batches produced by feature build can move to GPU dataframes for preprocessing without leaving columnar format. Worth a spike to confirm the zero-copy boundary holds end-to-end.

11. **Model serving fed Arrow batches (ONNX Runtime / Candle).** _(medium)_
    Close the loop: features as Arrow → tensor input for inference, in-process, in the same Rust runtime that ran the DataFusion query. Demonstrates the "no conversion from feature to prediction" claim.

### Tier 3 — Platform / ecosystem plays

Bigger bets that only make sense once Tiers 1–2 prove value.

12. **Distributed execution with Ballista.** _(large)_
    Same DataFusion plans and optimiser, split across nodes. Only worth it when single-node DataFusion provably runs out of headroom — premature otherwise.

13. **Streaming ingestion onto the substrate.** _(large)_
    Arrow + Kafka (or an engine like Arroyo) to land events as Arrow/Parquet continuously, feeding the "near-real-time features" use case with fresh data rather than hourly batches.

14. **ADBC drivers as they mature.** _(watch)_
    Arrow-native database connectivity (the ODBC/JDBC replacement). No mature Rust client yet; track it as the eventual clean path for warehouse round-trips that today go through `COPY INTO`.

15. **Schema registry / data contracts.** _(medium)_
    A central, versioned source of truth for Arrow schemas shared across feature build, training, and serving. The natural home for the "one schema everywhere" guarantee once more than one service depends on it.

---

## Suggested order of attack

If the goal is to convert this pilot into something an ML-artefact company actually relies on, a defensible sequence:

```
1. Promote examples → lib.rs            (Tier 1.5)   unblocks everything downstream
2. PyO3 bridge pilot                    (Tier 1.3)   proves the skew-killer claim
3. Validation-as-SQL                    (Tier 2.9)   cheap, immediately useful, low risk
4. Lance pilot                          (Tier 2.6)   the ML-specific differentiator
5. TableProvider + S3 + row-level sec.  (Tier 1.1)   the "beginning of a product"
```

Each step is an afternoon-to-a-week of work, produces a runnable artefact, and de-risks the next. The genesis doc's closing line holds: the first few steps take an afternoon; the later ones are the beginning of a product.

---

## Reading (beyond `genisis.md`'s list)

- Lance format design docs (lancedb.github.io) — why a new columnar format for ML, and how random-access reads differ from Parquet.
- "Data Validation for Machine Learning" (Breck et al., Google, 2019) — the case for validation as a first-class, declarative pipeline stage.
- Uber's Michelangelo / the feature-store literature — the canonical statement of the training/serving skew problem this stack is positioned to solve.
- Delta Lake and Apache Iceberg spec docs — table formats, ACID logs, and time travel over Parquet.
- DataFusion `TableProvider` examples in the DataFusion repo — the concrete API for Tier 1, item 1.
