
Benchmarking databases is notoriously difficult to do fairly. Snowflake and PostgreSQL are architecturally different systems designed for different workloads, so direct comparison is inherently asymmetric — like benchmarking a lorry against a sports car. The numbers below are drawn from published benchmarks, vendor documentation, community testing, and industry-standard TPC suites. Where possible, sources are cited. All figures should be treated as indicative ranges rather than absolute values, as performance varies significantly with hardware, configuration, data distribution, and query complexity.

---

## 1. OLTP — Transactional Workloads (Single-Row Operations)

This is where PostgreSQL dominates and Snowflake is fundamentally unsuited.

### Point Lookups (SELECT by Primary Key)

| Metric                                      | PostgreSQL                             | Snowflake                                                                             |
| ------------------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------- |
| Latency (indexed PK lookup)                 | **0.1–2ms**                            | **150–500ms**                                                                         |
| Throughput (point selects/sec, single node) | **Up to 2M QPS** (select-only, tuned)  | Not designed for this pattern                                                         |
| Latency floor                               | Sub-millisecond with warm buffer cache | ~100–200ms minimum overhead per query (cloud services layer compilation + scheduling) |
|                                             |                                        |                                                                                       |

**Sources:**

- PostgreSQL's pgbench record: **2M point-select QPS** and **137K write TPS** on tuned hardware (Vonng/pgtpc benchmark suite, GitHub). The write TPS figure represents TPC-B-like transactions where each transaction consists of 5 queries (SELECT, 3 UPDATEs, INSERT).
- Typical pgbench results on modest hardware (2GB RAM, 10 clients): **2,400–2,700 TPS** for TPC-B mixed read/write (CloudBees pgbench tuning guide). With tuning (shared_buffers, WAL settings), this rises to **3,000–5,000+ TPS** on the same hardware.
- Snowflake has an irreducible per-query overhead due to its cloud services layer (query parsing, optimisation, scheduling, result assembly). Even a `SELECT 1` typically takes **100–200ms**. This is architectural — it's the cost of distributed query planning.

**Verdict:** PostgreSQL is **100–1,000x faster** for point lookups. Snowflake should never be used for this workload.

This is done consistently across all the applications. Quotes being searched for, products being searched for in inventory or markdown. 

---

### Single-Row Inserts

|Metric|PostgreSQL|Snowflake|
|---|---|---|
|Single INSERT latency|**0.1–1ms**|**200ms–12 seconds** (row-by-row scripting)|
|Bulk INSERT (COPY, millions of rows)|**50,000–200,000 rows/sec** (depends on row width and hardware)|**Millions of rows/sec** via COPY INTO from staged files|

**Sources:**

- Snowflake performance testing showed that inserting **10 rows one-by-one via a scripting loop took 12 seconds**, while bulk-inserting **3 million rows in a single COPY took 2 seconds** (Analytics Today, "Snowflake Performance Tuning: Top 10 Tips"). This demonstrates Snowflake's batch-optimised architecture — row-by-row operations are an anti-pattern.
- PostgreSQL handles individual INSERTs in sub-millisecond time with write-ahead logging. Bulk COPY operations are efficient but single-node bound.

**Verdict:** PostgreSQL is **orders of magnitude faster** for individual writes. Snowflake wins on bulk loading from staged files at scale (parallel ingestion from cloud storage). Snowflake now supports ingestion rates up to **10 GB/second** with data queryable within 10 seconds (Snowflake BUILD 2025 announcement, NAND Research).

This happens constantly in practice — creating quotes, and anywhere an app saves, configures, or changes data from users. 

---

### Mixed OLTP (TPC-B / TPC-C)

|Metric|PostgreSQL|Snowflake|
|---|---|---|
|TPC-B TPS (10 clients, modest hardware)|**2,400–5,000 TPS**|Not applicable|
|TPC-B TPS (high-end, 64+ cores, tuned)|**50,000–137,000 TPS**|Not applicable|
|TPC-C (HammerDB, 16 vCPU managed)|**10,000–50,000 TPM** (varies by provider)|Not applicable|

**Sources:**

- pgbench baseline on 2GB RAM / 2 cores: **2,394 TPS**, rising to **2,662 TPS** after shared_buffers tuning (CloudBees).
- pgbench with 300 concurrent clients via PgBouncer (transaction mode): **~4,500 TPS** at 1.4ms average latency (Medium, Mehman Jafarov pgbench benchmarking article, 2025).
- High-end PostgreSQL benchmarks: **2M point-select QPS, 137K write TPS** (Vonng/pgtpc, GitHub).
- Google Cloud's AlloyDB (PostgreSQL-compatible) benchmarks TPC-C at scale on 16 vCPU instances, documenting the methodology (Google Cloud AlloyDB OLTP Benchmark documentation).
- Snowflake does not compete in OLTP benchmarks. Its architecture adds minimum ~200ms per query, making it unsuitable for high-frequency transactional workloads.

**Verdict:** This is entirely PostgreSQL's domain. Snowflake is not designed for OLTP.

---

## 2. OLAP — Analytical Workloads (TPC-H / TPC-DS)

This is where Snowflake dominates and PostgreSQL struggles at scale.

### TPC-H (22 Decision Support Queries)

|Scale|PostgreSQL (single node)|Snowflake|
|---|---|---|
|**1 GB (SF1)**|22 queries in **~5–15 seconds total** (warm cache, parallelism enabled)|22 queries in **well under 1 second each** on X-Small warehouse; sub-second total with result cache|
|**10 GB (SF10)**|22 queries in **~2–10 minutes total** (depending on parallelism, tuning)|22 queries in **seconds** on Small warehouse|
|**100 GB (SF100)**|22 queries in **22–80+ minutes total** (single node, 10-core laptop)|22 queries in **~30–120 seconds total** on Medium warehouse|
|**1 TB (SF1000)**|**Impractical** on single node (hours, likely OOM on complex queries)|22 queries in **~2–5 minutes total** on Large warehouse|
|**100 TB**|**Not feasible** on single-node PostgreSQL|22 queries in **~60–120 minutes** on 4XL warehouse (Databricks vs Snowflake benchmark, 2021)|

**Sources:**

- PostgreSQL TPC-H at SF50/SF75: EDB's longitudinal benchmark across PostgreSQL versions 8.3–17 shows dramatic improvements from parallelism (introduced in 9.6), but total runtime for 22 queries at SF75 (~75GB) still runs into **tens of minutes on a 10-core machine** (EDB, "TPC-H Performance Journey & Optimizations Since PostgreSQL 8.3").
- Vonng/pgtpc: PostgreSQL completes TPC-H SF50 in **~22 minutes** and TPC-H SF100 in **~80 minutes** on a 10-core laptop (GitHub).
- Crunchy Data found that running TPC-H on Iceberg tables via PostgreSQL was **14x faster** than on standard indexed PostgreSQL tables, highlighting PostgreSQL's row-storage disadvantage for analytical scans (Crunchy Data Blog, "Running TPC-H Queries on Iceberg Tables from PostgreSQL").
- Snowflake TPC-H at SF1: Satori benchmarking found most queries returned **well under 1 second** on the smallest warehouse, with 4,800 analytical queries completing rapidly (Satori, "Benchmarking Snowflake Performance Using TPC-H", 2023).
- Snowflake point lookup on 600M rows with clustering: **88ms** to find a single record, scanning only 1.5MB of 16GB compressed data due to micro-partition pruning (DZone, "Snowflake Performance Tuning: Top 5 Best Practices").

### TPC-DS (99 Decision Support Queries, 1TB Scale)

|System|Configuration|Approximate Total Runtime|Cost|
|---|---|---|---|
|Snowflake|4XL warehouse|**~4,000–7,300 seconds** (varies by data layout)|~$267 (standard tier)|
|Snowflake (pre-baked dataset)|4XL warehouse|**~4,025 seconds**|~$267|
|Snowflake (fresh data load)|4XL warehouse|**~7,276 seconds** (fastest of 3 runs)|~$267|
|PostgreSQL (single node)|N/A|**Not feasible at 1TB** for all 99 queries|N/A|

**Sources:**

- Fivetran/Brooklyn Data Co. Warehouse Benchmark (2022): Ran 99 TPC-DS queries at 1TB scale across Snowflake, BigQuery, Redshift, Databricks, and Synapse. All warehouses delivered **"excellent execution speed, suitable for ad hoc, interactive querying."** Snowflake, BigQuery, and Databricks were in a **near-tie for performance** at comparable configurations (Fivetran, "Cloud Data Warehouse Benchmark", 2022, updated 2025).
- Databricks vs Snowflake dispute (2021): Databricks claimed their pre-baked Snowflake TPC-DS dataset ran in ~4,025 seconds, but loading fresh official TPC-DS data into Snowflake yielded **~7,276 seconds** on 4XL — a significant difference attributed to data layout optimisation (Databricks Blog, "Snowflake Claims Similar Price/Performance to Databricks, But Not So Fast!", 2021).
- Snowflake Gen2 warehouses (2025): Snowflake claims a **56% reduction in query completion time** based on TPC-DS benchmark testing with their new warehouse generation (NAND Research, "Snowflake BUILD 2025 Announcements").

**Verdict:** Snowflake is **10–100x faster** than single-node PostgreSQL for analytical queries at any meaningful scale. At 1TB+, PostgreSQL simply cannot compete without distributed extensions (Citus) or a different engine entirely.

You do not replace snowflake with Postgres, but the point is to highlight they are both databases, but they are for very different usecases.

---

## 3. Concurrency

### Concurrent Read Queries

|Metric|PostgreSQL|Snowflake|
|---|---|---|
|Practical concurrent connections|**200–500** (without PgBouncer); **thousands** with PgBouncer|**Effectively unlimited** (multi-cluster auto-scaling)|
|Performance degradation under concurrency|Degrades linearly as connections compete for shared buffers, CPU, I/O|Each virtual warehouse is isolated; multi-cluster scales horizontally|
|Concurrent analytical queries (large scans)|Severe degradation — queries compete for I/O and memory|**10–20 concurrent queries per warehouse** before queuing; auto-scale adds clusters|

**Sources:**

- PostgreSQL's process-per-connection model becomes expensive above ~200 concurrent connections. PgBouncer in transaction mode mitigates this but doesn't solve resource contention on the database side (PostgreSQL documentation; pgBouncer benchmarks from community).
- pgbench at 300 concurrent clients showed **~488 TPS** with average latency of **535ms** — a significant degradation from the **~4,500 TPS at 1.4ms** achievable with lower concurrency through connection pooling (Medium, pgbench benchmarking, 2025).
- Snowflake's architecture isolates workloads per warehouse and auto-scales clusters. A single warehouse can handle **10–20 concurrent queries** efficiently; beyond that, multi-cluster warehouses spin up additional clusters automatically (e6data, "Snowflake Query Optimization 2025").
- Snowflake's concurrency benchmark (November 2025) specifically tested TPC-DS under concurrent BI and operational analytics workloads, demonstrating **consistent low latency at scale** (Snowflake Engineering Blog, "Benchmarking Concurrent Workloads for Operational Analytics and BI Workloads").

**Verdict:** For transactional concurrency (many small queries), PostgreSQL with PgBouncer handles thousands of connections efficiently. For analytical concurrency (many large queries), Snowflake's multi-cluster architecture is fundamentally superior.

---

## 4. Data Loading / Ingestion

|Metric|PostgreSQL|Snowflake|
|---|---|---|
|COPY from CSV (bulk load)|**50,000–200,000 rows/sec** (single-node, depends on row width)|**Millions of rows/sec** (parallel from cloud storage)|
|Streaming ingestion|Logical replication: **thousands of rows/sec**|Snowpipe Streaming: **up to 10 GB/sec**, queryable within 10 seconds|
|Row-by-row INSERT|**10,000–50,000 rows/sec** (batched in transactions)|**Anti-pattern** — ~1 row/sec effective due to per-query overhead|
|File format support|CSV, binary COPY|CSV, JSON, Avro, Parquet, ORC, XML (native)|

**Sources:**

- PostgreSQL COPY performance varies widely by hardware and row width, but **100K–200K rows/sec** is typical for moderate-width rows on modern hardware.
- Snowflake BUILD 2025: ingestion rates up to **10 GB/second** with sub-10-second query availability (NAND Research).
- Snowflake row-by-row insert benchmark: 10 rows via scripting loop = **12 seconds**; 3M rows via bulk COPY = **2 seconds** (Analytics Today).

**Verdict:** PostgreSQL wins for streaming single-row writes (application backends). Snowflake wins massively for bulk/batch loading from files at scale.

The above highlights why I see the benchmarks coming out of the Ingestion API as utterly abysmal. 

One benchmark I found was https://docs.databend.com/guides/benchmark/data-ingest which had 600 million rows ingested in 695s by snowflake or 860k rows per second. An ingestion API Should be looking to hit at least 250k per second.

The API is currently on for their "historical" loading as:
1. 5000 payload size: 650k records in 30 minutes (131 requests, 14s average)
2. 5 concurrent load: 1.6M records in 5 minutes (335 requests)
3. 30 minute test: 10M records (2K requests, 1.1 req/sec)
4. 8000 payload size: 1.7M records, 100% success rate

The gap is large, orders of magnitude large, I am looking to get the time to verify this myself at some point. 

---

## 5. Aggregation & Scan Performance

|Metric|PostgreSQL (single node)|Snowflake|
|---|---|---|
|Full table scan, 100M rows|**30–120 seconds** (row-based, depends on columns selected)|**1–5 seconds** (columnar, parallel, compressed)|
|COUNT(*) on 1B rows|**Minutes** (must scan all heap pages)|**Seconds** (metadata-level or highly parallel scan)|
|GROUP BY with aggregation, 1B rows|**Minutes to hours**|**Seconds to minutes** (depends on warehouse size)|
|JOIN two 100M-row tables|**Minutes** (hash join, single-node memory pressure)|**Seconds** (distributed hash join across cluster nodes)|

**Sources:**

- PostgreSQL's row-oriented storage means full table scans read every column even when the query only needs a few. This is the fundamental architectural disadvantage for analytical workloads.
- Snowflake's columnar storage + micro-partition pruning means it only reads the columns and partitions relevant to the query. The 600M-row clustered lookup example scanned **1.5MB of 16GB** (DZone).
- Snowflake result cache returns identical repeat queries in **milliseconds at zero compute cost** for 24 hours (e6data, Snowflake documentation).

**Verdict:** Snowflake is **10–100x faster** for large scans and aggregations, with the gap widening as data size increases.

Highlights the need to have both when looking to provide snappy frontends and services along with BI functionality. You would also pre-compute in Snowflake, then show in Postgres with next to 0 latency. 

---

## 6. Join Performance

|Join Type|PostgreSQL|Snowflake|
|---|---|---|
|Indexed nested-loop (small result from large table)|**Sub-millisecond to milliseconds** — this is PostgreSQL's strength|**200ms+ minimum** per query overhead|
|Hash join (two large tables)|Single-node; limited by memory; spills to disk above work_mem|Distributed across cluster nodes; scales with warehouse size|
|Sort-merge join (pre-sorted data)|Efficient if data fits in memory|Automatic; benefits from clustering keys|
|Cross-database join|Via FDWs (slow, network-dependent)|Native across databases within account; zero-copy with data sharing|

**Sources:**

- PostgreSQL's join performance is excellent for indexed joins and moderate-size hash joins. It degrades when work_mem is exceeded and joins spill to disk.
- Snowflake distributes joins across cluster nodes. A real-world example showed a join on 100M rows that was spilling to disk went from **10+ minutes to 2 minutes** (5x speedup) after clustering the join key (e6data, "Snowflake Query Optimization 2025").

**Verdict:** PostgreSQL wins for small, indexed joins (OLTP patterns). Snowflake wins for large analytical joins.

---

## 7. Semi-Structured Data (JSON)

|Metric|PostgreSQL (JSONB)|Snowflake (VARIANT)|
|---|---|---|
|Point lookup on JSON field (indexed)|**1–5ms** with GIN index|**200ms+** (query overhead)|
|Analytical scan across JSON field, 100M rows|**Minutes** (row storage, full scan)|**Seconds** (columnar, auto-flattened)|
|JSON ingestion|Parse on INSERT; validated|Parse on COPY; automatic schema detection|

**Sources:**

- PostgreSQL's JSONB with GIN indexes provides fast point access to semi-structured data but doesn't benefit from columnar storage for analytical scans.
- Snowflake's VARIANT type automatically flattens semi-structured data into columnar storage, making analytical queries over JSON, Avro, and Parquet data highly efficient.

**Verdict:** PostgreSQL wins for transactional access to JSON documents. Snowflake wins for analytical queries across large volumes of semi-structured data.

---

## 8. Text Search

|Metric|PostgreSQL|Snowflake|
|---|---|---|
|Full-text search (tsvector)|**1–50ms** with GIN index|Not natively supported (basic LIKE/ILIKE only)|
|Fuzzy search (pg_trgm)|**5–100ms** with trigram GIN index|Not supported natively|
|LIKE '%pattern%'|**Fast with pg_trgm index**|Full scan; Search Optimization Service can help for equality/substring|

**Sources:**

- PostgreSQL's built-in full-text search with tsvector/tsquery and pg_trgm for fuzzy matching is a significant capability that Snowflake lacks.
- Snowflake's Search Optimization Service improves point lookup and substring searches but is not a full-text search engine (Snowflake documentation, "Optimizing storage for performance").

**Verdict:** PostgreSQL wins decisively for text search workloads.

I don't see Peak needed this, but might be wrong. 

---

## 9. Geospatial

|Metric|PostgreSQL (PostGIS)|Snowflake|
|---|---|---|
|Spatial index lookup (within radius)|**1–10ms** with R-tree/GiST index|**200ms+** per query; basic GEOGRAPHY/GEOMETRY support|
|Complex spatial operations (intersection, buffer, routing)|Hundreds of specialised functions|Limited function set|
|Raster processing|Supported|Not supported|

**Sources:**

- PostGIS is the industry standard for geospatial data processing. Snowflake added GEOGRAPHY and GEOMETRY types but with a much smaller function set and no spatial indexing equivalent.

**Verdict:** PostgreSQL (PostGIS) wins overwhelmingly for geospatial workloads.

---

## 10. Caching Behaviour

|Cache Type|PostgreSQL|Snowflake|
|---|---|---|
|Buffer/page cache|**shared_buffers** (typically 25% of RAM) + OS page cache|Local SSD cache on warehouse nodes|
|Result cache|None built-in (application-level)|**24-hour result cache** — identical queries return in milliseconds at zero compute cost|
|Metadata cache|System catalog cache|Cloud services layer caches metadata, query plans|
|Cache effectiveness|Excellent for repeated access to hot data; cold starts are slow|Result cache is transformative for dashboards; first-run queries are slower|

**Sources:**

- Snowflake's result cache means that repeated dashboard queries (e.g., 5 users running the same sales report) cost nothing after the first execution and return in **milliseconds** (e6data; Snowflake documentation).
- PostgreSQL's shared_buffers and OS page cache are effective for working-set data but don't cache query results.
- pg_prewarm can pre-load tables into cache after restart, but there's no equivalent to Snowflake's automatic 24-hour result cache.

**Verdict:** Snowflake's result cache is a significant advantage for BI/dashboard workloads with repeated queries. PostgreSQL's buffer cache is more effective for transactional hot-data access patterns.

---

## Summary Benchmark Matrix

|Workload Type|PostgreSQL|Snowflake|Winner|Magnitude|
|---|---|---|---|---|
|**Point lookups (by PK)**|0.1–2ms|150–500ms|PostgreSQL|**100–1,000x**|
|**Single-row INSERT**|0.1–1ms|200ms–12s|PostgreSQL|**200–12,000x**|
|**Mixed OLTP (TPC-B)**|2,400–137,000 TPS|Not applicable|PostgreSQL|N/A|
|**Bulk data loading**|50K–200K rows/sec|Millions of rows/sec|Snowflake|**10–50x**|
|**Full table scan (100M rows)**|30–120s|1–5s|Snowflake|**10–100x**|
|**TPC-H 22 queries (10GB)**|2–10 min|Seconds|Snowflake|**10–50x**|
|**TPC-H 22 queries (100GB)**|22–80 min|30–120s|Snowflake|**20–100x**|
|**TPC-DS 99 queries (1TB)**|Not feasible|~60–120 min|Snowflake|∞|
|**Concurrent analytics (50+ users)**|Severe degradation|Auto-scales|Snowflake|**Architectural**|
|**Concurrent OLTP (1000+ conn)**|Handled with PgBouncer|Not designed for this|PostgreSQL|**Architectural**|
|**Full-text search**|1–50ms (indexed)|Not supported|PostgreSQL|∞|
|**Geospatial queries**|1–10ms (PostGIS indexed)|200ms+ (limited)|PostgreSQL|**100x+**|
|**JSON point access**|1–5ms (GIN indexed)|200ms+|PostgreSQL|**50–200x**|
|**JSON analytical scan (100M rows)**|Minutes|Seconds|Snowflake|**10–50x**|
|**Repeated query (result cache)**|No built-in cache|Milliseconds (free)|Snowflake|∞|

---

## Sources & References

1. **Vonng/pgtpc** (GitHub) — PostgreSQL TPC-B/C/H benchmarks. Record: 2M point-select QPS, 137K write TPS.
2. **CloudBees** — "Tuning PostgreSQL with pgbench". Baseline 2,394 TPS → 2,662 TPS after tuning.
3. **Mehman Jafarov** (Medium, 2025) — pgbench with PgBouncer: 300 clients, ~488 TPS at 535ms latency.
4. **DZone** — "How to Benchmark PostgreSQL for Optimal Performance" (2024). 1,420 TPS on moderate hardware.
5. **EDB** — "TPC-H Performance Journey & Optimizations Since PostgreSQL 8.3". Longitudinal TPC-H results across PG versions.
6. **Vonng/pgtpc** (GitHub) — PostgreSQL TPC-H: SF50 in ~22 min, SF100 in ~80 min on 10-core laptop.
7. **Crunchy Data Blog** — "Running TPC-H Queries on Iceberg Tables from PostgreSQL". Iceberg tables 14x faster than standard PG.
8. **Satori** — "Benchmarking Snowflake Performance Using TPC-H" (2023). TPC-H SF1 queries in well under 1 second.
9. **Analytics Today** — "Snowflake Performance Tuning: Top 10 Tips". Row-by-row insert: 10 rows = 12s. Bulk insert: 3M rows = 2s.
10. **DZone** — "Snowflake Performance Tuning: Top 5 Best Practices". Point lookup on 600M rows: 88ms with clustering.
11. **Fivetran / Brooklyn Data Co.** — "Cloud Data Warehouse Benchmark" (2022, updated 2025). TPC-DS 1TB across 5 warehouses.
12. **Databricks Blog** — "Snowflake Claims Similar Price/Performance" (2021). TPC-DS 100TB: Snowflake fresh data ~7,276s on 4XL.
13. **Barcelona Supercomputing Center** — TPC-DS derived benchmark: Databricks 2.7x faster than Snowflake at 100TB.
14. **NAND Research** — "Snowflake BUILD 2025 Announcements". 56% query time reduction; 10 GB/s ingestion.
15. **e6data** — "Snowflake Query Optimization 2025". Join on 100M rows: 10+ min → 2 min after clustering.
16. **Snowflake Documentation** — Search Optimization Service, result caching, virtual warehouse architecture.
17. **Google Cloud** — "Benchmark OLTP performance on AlloyDB for PostgreSQL". TPC-B and TPC-C methodology.
18. **Benchant** — "PostgreSQL DBaaS Query Performance". TPC-H and TATP across AWS, Azure, GCP managed PostgreSQL.
19. **PostgreSQL Wiki** — TPC-H page. Notes on missing features and parallel query improvements.
20. **Snowflake Engineering Blog** — "Benchmarking Concurrent Workloads" (November 2025). Concurrency under TPC-DS.

---

## The Bottom Line

The benchmarks confirm what the architecture predicts:

- **PostgreSQL** is **100–1,000x faster** for transactional operations (point lookups, single-row writes, indexed joins, text search, geospatial). Its performance ceiling is the single-node hardware limit.
    
- **Snowflake** is **10–100x faster** for analytical operations (large scans, aggregations, complex joins over massive datasets), with the gap widening as data size increases. Its performance scales linearly with compute spend.
    
- There is **no overlap zone** where both are competitive — they are architecturally optimised for different workloads. The ~200ms minimum query overhead in Snowflake makes it permanently unsuitable for OLTP, while PostgreSQL's single-node, row-oriented architecture makes it permanently unsuitable for multi-terabyte analytics.
    

This is why the most effective production architectures use both: PostgreSQL for the application layer, Snowflake for the analytical layer.