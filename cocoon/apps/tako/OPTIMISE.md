# Optimising `parse.rs`

Performance notes for the file-decode hot path (`src/parse.rs`). The code is
correct and readable; the wins come from **doing less work** — skipping schema
inference, copies, and format round-trips — rather than from micro-optimising the
Rust itself.

> **Measure first.** A Criterion bench already exists:
> `cargo run --features testing -- bench-parse`. Every change below should be
> validated against it — intuition about columnar decoding is often wrong. The
> parse already runs inside the upload handler's `spawn_blocking`, so it is off
> the async workers; that part needs no change.

---

## Tier 1 — the real gains

### 1. Pass the known schema to the CSV/JSON readers

The cleanest and highest-value change, because tako **already** has the
`TableSchema` (and `TableSchema::to_polars_schema()`).

Today `parse_data` does not know the schema, so `CsvReader` / `JsonReader` run a
**schema-inference pass** (scanning ~100 rows to guess column types) before
decoding. Supplying the schema (`CsvReadOptions::with_schema(Some(schema))`):

- removes the inference pass entirely;
- yields correctly-typed columns directly, so `validate_and_reorder` has less to
  do and mis-inference (an `id` guessed as `f64`) disappears.

Biggest payoff on CSV and JSON (row-oriented). Parquet/Avro already carry their
schema.

**Cost:** thread the Polars `Schema` into `parse_data` (a signature change). The
schema is already loaded in the upload closure, so it is available at the call
site.

### 2. Vortex — drop the IPC serialise→deserialise round-trip

The Vortex path is: Vortex array → arrow-rs `StructArray` → **IPC bytes** →
Polars `IpcStreamReader`. That round-trip **encodes then re-parses the entire
dataset** through a buffer — a full copy + encode + decode.

The faster route is the **Arrow C Data Interface (FFI)**: a zero-copy import of
the arrow-rs array into polars-arrow, with no serialisation. That is exactly what
the Arrow FFI exists for (cross-implementation exchange).

**Caveat (honest):** FFI is `unsafe` and requires compatible Arrow versions
between arrow-rs and polars-arrow — more delicate than the current IPC bridge.
IPC is the pragmatic choice; FFI is the upgrade once profiling shows Vortex is
hot.

### 3. Vortex — avoid `data.to_vec()`

`open_buffer(data.to_vec())` **clones the whole payload** (100 MB → another
100 MB) because `parse_data` takes `&[u8]`. The upstream `data` is already an
owned `Bytes`. Threading owned `Bytes` (or `Arc<[u8]>`) down to here would let
`open_buffer` consume it without the copy.

---

## Tier 2 — projection

### 4. Projection pushdown (Parquet especially)

Today every column in the file is decoded, then `validate_and_reorder` does
`df.select(...)` to keep only the schema's columns. For a wide Parquet file (many
columns, schema wants few) that is decoding thrown away.
`ParquetReader::with_projection(...)`, derived from the schema's columns, decodes
only what is needed — the gain scales with the fraction of columns ignored.

---

## Tier 3 — structural (only if profiling demands it)

- **Lazy / streaming** (`scan_parquet` + `collect`): lets Polars push projection
  and predicate down and cap memory per chunk — pairs with the per-upload memory
  ceiling. The input is an in-memory buffer, though, so the main benefit here is
  the projection (already #4) and a lower memory peak, not true streaming I/O.
- **Thread contention:** Polars parallelises via rayon. Under high concurrency
  (many simultaneous uploads), the rayon pool plus the `spawn_blocking` threads
  can oversubscribe the cores. The upload semaphore already bounds this;
  `POLARS_MAX_THREADS` is the knob if queue latency degrades.

---

## Beyond parse — the upload request path

`parse` is one stage. The full `/upload` hot path is:

```
acquire permit → buffer body (Bytes) → [spawn_blocking: resolve schema → parse
→ validate/reorder → DV-enrich → encode Parquet (Vec<u8>)] → upload to S3
→ [spawn_blocking: COPY from S3] → 200
```

**Know where the time goes — it differs by backend.** For Redshift/Snowflake the
**warehouse-side `COPY`** (the warehouse reading the object back out of S3)
dominates wall-clock; tako's parse/encode is marginal to total latency, so the
win there is *throughput*, not per-request speed. For local DuckDB, tako's own
CPU (parse + Parquet encode) dominates, so the `parse` tiers above are what move
the needle. Profile per backend before optimising.

### U1. Cache the schema as `Arc<TableSchema>` in the registry

`SchemaRegistry::get_or_load` caches in memory (good — no per-request disk I/O),
but `get()` returns `.cloned()`, so **every upload deep-clones the whole
`TableSchema`** (its `Vec<ColumnDef>`, constraints, key declarations). For a wide
schema that is a real per-request allocation. Caching and returning
`Arc<TableSchema>` makes the lookup a refcount bump. Small, but it is on every
request. Cheap win.

### U2. Pick a fast Parquet codec deliberately

The Parquet here is a **transient staging object**: written, `COPY`ed, then it has
done its job. So optimise for encode speed and transfer size, not maximum
compression — prefer Snappy / LZ4 (`ParquetWriter::with_compression(...)`) over
Zstd-at-a-high-level. Measure the three-way trade-off: encode CPU (tako) vs S3
transfer vs warehouse `COPY` time. Cheap win once measured.

### U3. Stream the Parquet to S3 instead of buffering then uploading

Today the encode produces a full `Vec<u8>` in RAM, which is then handed to
`upload_large_object`. A streaming / multipart write would overlap encoding with
the upload and remove the Parquet buffer from the peak — this is the same
`Parquet-to-S3 streaming` lever noted in the memory-ceiling work. Bigger change;
depends on the blob client exposing a streaming/multipart API.

### U4. Throughput ceiling — the long-held DB connection

Each upload holds a pooled warehouse connection for the **entire** `COPY` (seconds
to minutes on Redshift). Beyond the pool size, requests queue. This caps
concurrent throughput (not single-request latency). The structural fixes are the
async-driver migration and/or the async job-queue pattern (both documented
elsewhere). Listed here for completeness — it is the throughput ceiling, and no
amount of parse/encode tuning moves it.

---

## Not worth touching

- `FileType::from_file_name` — the `to_lowercase()` allocates, but it runs once
  per upload. Negligible.
- `parse_json`'s array-vs-NDJSON detection — `O(leading whitespace)`, trivial.
- The `spawn_blocking` offload — already correct.

---

## Recommended order

By effort-to-gain (parse `#` and upload-path `U#` items interleaved):

1. **U1 — schema as `Arc`.** Tiny diff, every request, no behaviour change.
2. **#1 — schema to the readers.** Clean, low-risk, and improves robustness
   (no mis-inference). Integrates with the upload closure that already loads the
   schema.
3. **U2 — fast Parquet codec.** One-line knob; measure the trade-off first.
4. **#3 — Vortex copy** and **#4 — Parquet projection.**
5. **U3 — stream Parquet to S3** (memory + latency) and **#2 — Vortex FFI** —
   the larger changes, for when profiling justifies them.

**U4** (the long-held connection) is a throughput ceiling addressed by the async
migration, not a parse/encode tweak — track it with that work, not here.
