## Arrow: The Memory Format That Changed Everything

Before Arrow, every analytical tool had its own in-memory data format. Pandas had its internal representation. Spark had its Row format (then Tungsten). R had data frames. Every tool that needed to talk to another tool paid a serialisation tax — convert from format A to bytes, send bytes, convert bytes to format B. For large datasets, this serialisation dominated total processing time.

Apache Arrow defines a single columnar memory format that every tool agrees on. Not a file format (that's Parquet). Not a query language (that's SQL). A _memory layout_ — how columns of typed data sit in RAM. Integers are contiguous arrays. Strings are offset arrays. Nulls are bitmaps. Nested types (structs, lists, maps) compose from primitives.

The consequence: when two Arrow-aware systems exchange data, they exchange _pointers_, not copies. A Rust process can hand a batch of Arrow data to a Python process (via PyArrow) with zero serialisation. A DuckDB query result is Arrow-formatted and can be consumed by Polars, DataFusion, or any Arrow-native library without conversion. The serialisation tax drops to zero.

This isn't a minor optimisation. For the ML System Architecture — where features flow from batch computation through a feature store into real-time serving — Arrow as the common format means features computed in Python (batch training) and features served in Rust (real-time inference) share the same memory layout. No conversion, no subtle type mismatches, no serialisation bugs. The training/serving skew problem gets smaller when both sides speak the same data language.

## The Arrow Ecosystem

Arrow is a specification, but it's also a family of implementations:

**PyArrow (Python).** The most mature Arrow implementation. Integrates with Pandas (zero-copy conversion via `pa.Table.from_pandas()`), NumPy, and most Python data tools. Includes a Parquet reader/writer, CSV reader, IPC format for inter-process communication, and Flight for network data transfer.

**arrow-rs (Rust).** The Rust implementation. Used internally by DataFusion, Polars, and Delta Lake's Rust bindings. Provides Array types, RecordBatch (a collection of equal-length arrays with a schema), and compute kernels (sort, filter, aggregate, cast, arithmetic) that operate on Arrow arrays directly.

**Arrow Flight.** A gRPC-based protocol for transferring Arrow data over the network. Instead of serialising to JSON or CSV for an API response, Flight sends Arrow batches. The receiver gets typed columnar data with zero parsing. For a data product that serves analytical results to consumers, Flight replaces REST APIs with something dramatically more efficient for tabular data.

**Arrow Flight SQL.** Extends Flight with SQL semantics. A client sends a SQL query; the server returns Arrow batches. This is how DuckDB, DataFusion, and other engines expose their results to remote clients. It's a universal analytical API — any Flight SQL client talks to any Flight SQL server.

**Arrow IPC (Inter-Process Communication).** A format for sharing Arrow data between processes on the same machine. Memory-mapped files containing Arrow batches can be read by multiple processes simultaneously without copying. For the architecture where a Rust service computes features and a Python process trains on them, Arrow IPC files are the zero-copy bridge.

## DataFusion: The Query Engine as a Library

DuckDB is an embedded database. DataFusion is an embedded _query engine_. The distinction matters.

DuckDB manages storage, memory, transactions, and query execution. It's a complete database. DataFusion manages only query execution — planning, optimisation, and running queries against data sources you provide. It's a library of components you compose into a custom query engine.

DataFusion's components:

**SQL parser.** Parses SQL strings into logical plans. Supports standard SQL with extensions.

**Logical planner.** Converts parsed SQL into a logical plan (a tree of operations: scan, filter, project, join, aggregate). The logical plan describes _what_ to compute, not _how_.

**Optimiser.** Rewrites the logical plan for efficiency. Predicate pushdown (move filters closer to the data source), projection pruning (only read needed columns), join reordering, constant folding. The optimiser has 20+ rules that compose.

**Physical planner.** Converts the optimised logical plan into a physical plan — the actual execution strategy. Chooses hash joins vs merge joins, decides parallelism, selects sort algorithms based on data size.

**Execution engine.** Runs the physical plan, producing Arrow RecordBatches as output. Supports streaming execution (results flow through operators without materialising intermediate results in full), partitioned execution (operators run in parallel across partitions), and spilling to disk when memory is exceeded.

**Data source abstraction (TableProvider).** The interface between DataFusion and your data. Implement `TableProvider` for any data source — Parquet files, CSV files, PostgreSQL tables, REST APIs, custom formats — and DataFusion queries it. This is the composability hook: DataFusion doesn't care where data lives, only how to read it through the Arrow interface.

## Why This Matters: Building a Custom Analytical Layer

The product vision involves serving analytical results to customers. Currently, this means either querying Snowflake (expensive, latency-sensitive, rate-limited) or building bespoke reporting. DataFusion offers a third path: a custom analytical engine embedded in the Rust service layer.

**Scenario: Per-customer analytical endpoints.**

Each customer has data in Parquet (via the S3 shuttle). Instead of routing every analytical query to Snowflake:

1. Register each customer's Parquet files as DataFusion table sources
2. Apply row-level security (each customer sees only their data) in the TableProvider
3. Expose SQL or a domain-specific query language through an API
4. DataFusion plans, optimises, and executes the query
5. Results return as Arrow batches (via Flight) or JSON (via serialisation)

The query runs in-process in the Rust service. No Snowflake compute. No network round-trip to a warehouse. For queries against datasets under a few hundred GB, performance is comparable to Snowflake and latency is dramatically lower (milliseconds vs seconds).

**Scenario: Feature computation for ML serving.**

The ML System Architecture describes batch features precomputed and real-time features computed at request time. DataFusion can handle the in-between: near-real-time features that require analytical computation over recent data.

Register the last N hours of event data as a DataFusion table. Run feature queries (aggregations, window functions, joins against reference data) at request time. Results are Arrow arrays that feed directly into the model inference pipeline with zero conversion. This replaces the feature store for features that are too fresh for batch but too complex for simple Redis lookups.

**Scenario: Data validation as SQL.**

The DV2 validation layer checks schema conformance, uniqueness, referential integrity. These are naturally expressed as SQL queries: `SELECT * FROM incoming WHERE business_key IS NULL`, `SELECT business_key, COUNT(*) FROM incoming GROUP BY business_key HAVING COUNT(*) > 1`. DataFusion runs these against incoming data before it touches the warehouse. Validation becomes a set of SQL queries, not a set of procedural checks.

## The DataFusion Ecosystem

DataFusion is the engine. The ecosystem builds on it:

**Ballista.** Distributed DataFusion — splits query execution across multiple nodes. Like Spark but Rust-native and Arrow-native. Still maturing, but the trajectory is clear: DataFusion for single-node, Ballista for distributed, same query plans and optimiser.

**Delta Lake (delta-rs).** Delta Lake's Rust implementation uses Arrow and integrates with DataFusion. Query Delta tables (Parquet files with ACID transaction logs) directly through DataFusion. Time travel, schema evolution, and transactional writes on object storage.

**Iceberg (iceberg-rust).** Same as Delta Lake but for Apache Iceberg format. DataFusion can query Iceberg tables natively.

**Object Store crate.** Arrow's object store abstraction supports S3, GCS, Azure Blob, and local filesystem. DataFusion reads from any of these transparently. The S3 shuttle pattern means DataFusion reads the same Parquet files that Snowflake reads.

**Parquet crate.** Arrow's Parquet implementation supports predicate pushdown (read only rows matching a filter), projection pushdown (read only needed columns), page-level statistics (skip entire row groups that can't match), and bloom filters. DataFusion leverages all of these automatically when querying Parquet files.

## Arrow + Rust + Python: The Polyglot Bridge

The ML System Architecture describes a shared Rust feature engine with a PyO3 bridge to Python. Arrow makes this bridge nearly free:

```
Rust (arrow-rs) computes features as Arrow RecordBatches
    ↓ (zero-copy via PyO3 + PyArrow)
Python receives Arrow data as PyArrow Tables
    ↓ (zero-copy conversion)
Pandas/Polars/NumPy consumes the data for training
```

The data never leaves Arrow format. No JSON serialisation, no CSV round-tripping, no protobuf encoding. The bytes that Rust wrote are the bytes that Python reads. For feature pipelines moving millions of rows between training and serving, this eliminates an entire class of performance problems and type conversion bugs.

Going the other direction: Python computes training data as PyArrow Tables, writes them as Parquet, and the Rust serving engine reads them through DataFusion. The same schema, the same types, the same data — just different query engines appropriate for different contexts (Polars/Pandas for interactive analysis, DataFusion for production serving).

## Loading Into Warehouses: The S3 Shuttle

The natural question after computing Arrow data and writing Parquet: how does it get into Snowflake or Redshift?

Not via INSERT. Warehouses are optimised to ingest _files_, not individual rows. The pattern is:

```
Rust (arrow-rs) → write Parquet → S3 bucket → COPY INTO table
```

The Parquet file that DataFusion or ArrowWriter produces is _already_ in the format both warehouses expect. No CSV conversion, no JSON serialisation. The same columnar types flow end to end.

**Snowflake:**

```sql
COPY INTO sensors
  FROM @my_s3_stage/sensors.parquet
  FILE_FORMAT = (TYPE = PARQUET)
  MATCH_BY_COLUMN_NAME = CASE_INSENSITIVE;
```

**Redshift:**

```sql
COPY sensors
  FROM 's3://my-bucket/data/sensors.parquet'
  IAM_ROLE 'arn:aws:iam::role/RedshiftRole'
  FORMAT AS PARQUET;
```

**The S3 upload from Rust** uses `object_store`, which is already in the dependency tree via DataFusion:

```rust
use object_store::aws::AmazonS3Builder;
use object_store::ObjectStore;

let s3 = AmazonS3Builder::from_env()
    .with_bucket_name("my-bucket")
    .build()?;

let bytes = std::fs::read("data/sensors.parquet")?;
s3.put(&"data/sensors.parquet".into(), bytes.into()).await?;
```

**Why not INSERT?** A `COPY INTO` from Parquet on S3 loads millions of rows in seconds — the warehouse parallelises reads across its compute nodes. Row-by-row INSERT goes through network, SQL parsing, and WAL for each row. The performance difference is easily 100x.

**Other paths worth knowing about:**

- _Arrow Flight SQL_ — a gRPC protocol where the warehouse returns Arrow batches directly. Not widely supported yet but it's the direction the ecosystem is heading.
- _ADBC (Arrow Database Connectivity)_ — the Arrow-native replacement for ODBC/JDBC. Snowflake has a Python/C ADBC driver. No mature Rust client yet, but the spec exists.
- _Classic SQL connectors_ (`sqlx`, `tokio-postgres`) — fine for small volumes or metadata operations, not for bulk data.

The key insight: the examples in this project already produce warehouse-ready Parquet. Adding `object_store::put` to S3 and a `COPY INTO` on the warehouse side completes the pipeline. No format conversion at any stage — Arrow types map directly to Parquet types map directly to warehouse column types.

## Where This Is Going

Arrow and DataFusion are converging with Parquet and object storage to create a "universal analytical substrate" — a layer where data lives in Parquet on object storage, is queried through Arrow-native engines, and flows between services in Arrow format. DuckDB, Polars, DataFusion, Snowflake, BigQuery, and Databricks all participate in this ecosystem to varying degrees.

Positioning the stack on this substrate (Parquet files, Arrow in-memory format, DataFusion for embedded queries, Snowflake for heavy analytics) means every new tool that joins the Arrow ecosystem becomes immediately usable. It's investing in the interchange format, not any single engine.

## Getting Started

1. `cargo add datafusion` in a Rust project
2. Register a local Parquet file as a table
3. Run a SQL query, get Arrow RecordBatches back
4. Measure query latency against the same query on Snowflake
5. Then: implement a custom TableProvider that reads from S3 with customer-specific access controls

The first four steps take an afternoon. The fifth is the beginning of a product.

## Reading

- Apache Arrow specification (arrow.apache.org/docs/format/) — the memory format explained formally
- DataFusion architecture guide (docs.rs/datafusion) — how the planner, optimiser, and execution engine compose
- Wes McKinney's original Arrow blog post (2016) — the motivation for a universal columnar format, from Pandas' creator
- Andy Pavlo's CMU Database Group lectures on query execution — the computer science behind what DataFusion implements
- The "Lakehouse" paper by Databricks (2020) — the architectural vision that Arrow/Parquet/DataFusion implements at the open-source layer
