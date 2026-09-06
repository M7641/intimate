## The Core Principle

Neither database should query the other directly at request time. The synchronisation always happens asynchronously through an intermediate layer — event bus, cloud storage, or orchestrator. This keeps both systems performing at their best: PostgreSQL stays fast for transactions, Snowflake stays elastic for analytics, and the sync layer is independently observable, retryable, and scalable.

What follows is a comprehensive review of every viable pattern, including a dedicated analysis of the intermediate service + S3 shuttle approach and how it compares.

---

## Pattern 1: Change Data Capture (CDC)

**Direction:** PostgreSQL → Snowflake

**How it works:** Debezium reads PostgreSQL's write-ahead log (WAL) and emits every INSERT, UPDATE, and DELETE as a structured event. Those events flow through Kafka or NATS into cloud storage (S3/GCS/Azure Blob), and Snowpipe auto-ingests them into Snowflake.

**Latency:** 1–5 minutes end-to-end.

**Impact on PostgreSQL:** Minimal — reads the WAL rather than polling tables, so there's no additional query load on the primary.

**Operational complexity:** Medium-high. You're running Debezium, a message broker, and managing connector configuration, schema evolution, and offset tracking.

**Best for:** Continuous replication of operational data into the warehouse. The canonical PostgreSQL → Snowflake path for teams that need near-real-time freshness.

**Trade-offs:** WAL-based CDC captures every change, which can generate high event volumes on write-heavy tables. Schema changes in PostgreSQL need careful handling (Debezium supports schema evolution, but it requires configuration). Operational overhead of running the CDC stack is non-trivial.

---

## Pattern 2: Managed CDC (Fivetran / Airbyte)

**Direction:** PostgreSQL → Snowflake

**How it works:** Fivetran or Airbyte connects directly to PostgreSQL's logical replication slot, handles WAL decoding, transformation, and loading into Snowflake. Configuration happens through a UI.

**Latency:** As low as 1 minute (Fivetran) or 5 minutes (Airbyte).

**Impact on PostgreSQL:** Low — same WAL-based approach as Debezium, but managed.

**Operational complexity:** Low. The vendor handles infrastructure, retries, schema migration, and monitoring.

**Cost:** Fivetran charges per million rows synced (can get expensive on high-volume tables). Airbyte is open source and self-hostable, trading money for operational effort.

**Best for:** Teams that want CDC without running Debezium/Kafka. The path of least resistance for most organisations.

**Trade-offs:** Vendor lock-in (Fivetran), cost at scale, and less control over transformation logic before data lands in Snowflake. Airbyte self-hosted gives control back but adds operational burden.

---

## Pattern 3: Batch Export (COPY → Cloud Storage → Snowflake)

**Direction:** PostgreSQL → Snowflake

**How it works:** A scheduled job runs `COPY` from PostgreSQL to CSV or Parquet files on cloud storage (S3/GCS/Azure Blob). Snowflake's `COPY INTO` or Snowpipe picks up the files.

**Latency:** Whatever your schedule is — hourly, daily, weekly.

**Impact on PostgreSQL:** Moderate during the export window. The COPY command runs a sequential scan, which can compete with production queries on large tables. Mitigate by running against a read replica.

**Operational complexity:** Low. A cron job, pg_cron task, or orchestrator step. Minimal moving parts.

**Best for:** Reference tables, dimension tables, configuration data, or any dataset where daily/hourly freshness is sufficient. Also a good starting point before investing in CDC infrastructure.

**Trade-offs:** No incremental capture — you're exporting full snapshots or manually managing watermarks (e.g., `WHERE updated_at > last_export_time`). Watermark-based approaches miss deletes unless you use soft deletes. Coarse granularity compared to CDC.

---

## Pattern 4: Reverse ETL (Census / Hightouch / Polytomic)

**Direction:** Snowflake → PostgreSQL

**How it works:** These tools query Snowflake on a schedule, diff the results against what's already in PostgreSQL, and sync only the changes. They support upserts, deletes, and field-level mapping.

**Latency:** Minutes to hours, depending on schedule and dataset size.

**Operational complexity:** Low. Managed SaaS with a configuration UI.

**Best for:** Pushing analytical results, computed features, audience segments, or aggregated metrics from Snowflake back into the operational layer. The cleanest option for warehouse → application flows.

**Trade-offs:** Another vendor and cost centre. The diff logic can be slow on very large datasets. Limited transformation capability — you're expected to do transformation in Snowflake (via dbt) before the reverse ETL picks it up.

---

## Pattern 5: Custom Pipeline via Orchestrator

**Direction:** Snowflake → PostgreSQL (or bidirectional)

**How it works:** Dagster or Airflow runs a task that queries Snowflake (via the Python connector), transforms results in-memory or via Pandas/Polars, and writes to PostgreSQL (via psycopg, SQLAlchemy, or COPY). Full control over logic, scheduling, and error handling.

**Latency:** Depends on schedule and query complexity. Typically minutes to hours.

**Operational complexity:** Medium. You own the code, the orchestrator, and the failure modes.

**Best for:** Complex transformations, conditional sync logic, multi-step pipelines where the sync is one step in a larger workflow. Also the natural approach for the feature store pattern (Snowflake → compute features → write to Redis and PostgreSQL).

**Trade-offs:** You're writing and maintaining pipeline code. Schema changes require code updates. Error handling (retries, idempotency, dead-letter handling) is your responsibility.

---

## Pattern 6: Intermediate Service + S3 Shuttle

**Direction:** Bidirectional

This is the pattern worth examining in detail, as it offers a compelling middle ground between the simplicity of batch export and the sophistication of CDC — with the added benefit of working cleanly in both directions.

### How It Works

A dedicated service (the "shuttle") sits between PostgreSQL and Snowflake. It doesn't connect them directly — instead, it uses cloud object storage (S3, GCS, Azure Blob) as the intermediate staging layer. The shuttle service orchestrates queries against each database, writes results to storage, and triggers ingestion on the other side.

```
┌─────────────┐         ┌─────────────────┐         ┌─────────────┐
│  PostgreSQL  │◄───────►│  Shuttle Service │◄───────►│  Snowflake  │
└──────┬──────┘         └────────┬────────┘         └──────┬──────┘
       │                         │                         │
       │                    ┌────▼────┐                    │
       │                    │   S3    │                    │
       │                    │  (Parq- │                    │
       │                    │  uet)   │                    │
       │                    └─────────┘                    │
       │                         │                         │
       └─────── reads/writes ────┴──── reads/writes ───────┘
```

**PostgreSQL → Snowflake flow:**

1. Shuttle queries PostgreSQL (using watermarks, change tracking, or full snapshot).
2. Results are written to S3 as Parquet files (partitioned by date, entity, or batch ID).
3. Shuttle triggers Snowflake `COPY INTO` or Snowpipe picks up the files automatically.
4. Shuttle updates its watermark state (last synced ID, timestamp, or LSN).

**Snowflake → PostgreSQL flow:**

1. Shuttle queries Snowflake for computed results (features, aggregates, scores).
2. Results are written to S3 as Parquet files.
3. Shuttle reads the Parquet files and bulk-loads into PostgreSQL via `COPY`.
4. Alternatively, shuttle reads directly from the Snowflake query result and writes to PostgreSQL, using S3 only as a checkpoint/recovery mechanism.

### Why This Pattern Is Interesting

**Decoupled by design.** Neither database knows about the other. The shuttle service and S3 form a clean boundary. If Snowflake is down, PostgreSQL exports still land in S3 and will be picked up when Snowflake recovers. If PostgreSQL is under load, the shuttle can read from a replica. S3 acts as a durable buffer that absorbs timing mismatches.

**Parquet as the lingua franca.** By standardising on Parquet in S3, you get a format that both PostgreSQL (via COPY or parquet_fdw) and Snowflake (natively) read efficiently. You also get a free data lake as a side effect — the Parquet files in S3 are queryable by Athena, DuckDB, Spark, or anything else, giving you an escape hatch from both databases.

**Inspectable and replayable.** Every sync operation produces files in S3 with predictable paths (e.g., `s3://data-shuttle/pg-to-sf/orders/2026/02/18/batch-001.parquet`). If something goes wrong, you can inspect the files, replay a batch, or reprocess from a specific point. CDC event streams offer this too, but Parquet files are easier to reason about and debug.

**Bidirectional with one pattern.** The same service, the same S3 bucket structure, and the same Parquet format work in both directions. You're not running Debezium for one direction and a reverse ETL tool for the other — it's a single, consistent mechanism.

**Schema flexibility.** The shuttle service can handle schema mapping, type conversion, and transformation between PostgreSQL and Snowflake's type systems (e.g., PostgreSQL's `JSONB` → Snowflake's `VARIANT`, PostgreSQL's `TIMESTAMPTZ` → Snowflake's `TIMESTAMP_TZ`). This logic lives in one place rather than being spread across CDC connectors and dbt models.

### Implementation Considerations

**The shuttle service itself** can be surprisingly simple. A Rust service (for performance and reliability) or a Python service (for ecosystem convenience) that:

- Maintains a state table (in PostgreSQL or a simple SQLite/Redis) tracking the last synced position for each table/flow.
- Runs on a schedule (triggered by pg_cron, NATS event, or the orchestrator) or continuously as a long-running process.
- Uses connection pooling to both PostgreSQL and Snowflake.
- Writes Parquet via the `arrow` crate (Rust) or `pyarrow` (Python) — both are fast and produce Snowflake-compatible files.
- Uploads to S3 via the AWS SDK with multipart upload for large batches.

**Watermark strategies for incremental sync:**

- `updated_at` timestamp column — simple, works for upserts, misses hard deletes.
- Auto-incrementing ID — works for append-only tables, doesn't capture updates.
- PostgreSQL logical replication slot — the shuttle reads the WAL directly (essentially building a lightweight Debezium). More complex but captures everything.
- Snowflake streams — for the Snowflake → PostgreSQL direction, Snowflake's native change tracking (streams) gives you a clean incremental feed.

**File organisation in S3:**

```
s3://data-shuttle/
  ├── pg-to-sf/
  │   ├── orders/
  │   │   ├── 2026/02/18/
  │   │   │   ├── batch-001.parquet
  │   │   │   └── batch-002.parquet
  │   │   └── _watermark.json
  │   └── users/
  │       └── ...
  └── sf-to-pg/
      ├── user_features/
      │   └── 2026/02/18/
      │       └── batch-001.parquet
      └── model_scores/
          └── ...
```

**Error handling and idempotency:** Each batch gets a unique ID. The shuttle writes the batch to S3, then updates the watermark atomically. If it crashes mid-batch, the incomplete Parquet file in S3 is either overwritten on retry or ignored (Snowpipe can be configured to skip already-loaded files). PostgreSQL-side loads should use staging tables with upsert (`INSERT ... ON CONFLICT`) for idempotency.

### When to Choose the S3 Shuttle Over Other Patterns

The shuttle pattern sits in a specific sweet spot. Here's how it compares:

**vs CDC (Debezium):**

- Shuttle is simpler operationally — no Kafka/NATS dependency for the sync itself (though it can use them for triggering).
- CDC captures every individual change; the shuttle batches changes into periodic snapshots. If you need every single mutation in order (event sourcing, audit trail), CDC is better. If you need "give me the current state every N minutes," the shuttle is cleaner.
- CDC is strictly PostgreSQL → downstream. The shuttle works bidirectionally with the same pattern.
- CDC handles deletes natively (tombstone events). The shuttle needs soft deletes or periodic full snapshots to catch deletes.

**vs Managed CDC (Fivetran/Airbyte):**

- Shuttle gives you full control and no per-row pricing. At high volumes, the cost difference is significant.
- Managed tools are faster to set up and require no code. The shuttle requires building and maintaining a service.
- Managed tools handle schema evolution automatically. The shuttle needs explicit handling.
- The shuttle gives you the S3 data lake as a free side effect. Managed tools write directly to Snowflake.

**vs Reverse ETL (Census/Hightouch):**

- For Snowflake → PostgreSQL specifically, reverse ETL tools are more polished and handle diffing automatically.
- The shuttle is more flexible — you control the query, the transformation, and the loading logic.
- The shuttle avoids another SaaS vendor and cost centre.
- If you need both directions, the shuttle is one system; reverse ETL only covers one direction.

**vs Custom Orchestrator Pipeline:**

- Very similar in spirit. The key difference is that the shuttle uses S3 as an explicit intermediate layer, which gives you the data lake side effect, inspectability, and decoupling benefits.
- A pure orchestrator pipeline (Snowflake → Python → PostgreSQL) keeps data in memory or temp files. The shuttle's S3 stage makes it more resilient to failures and easier to debug.
- The shuttle can run independently of the orchestrator (though it works well as an orchestrator-triggered task too).

### Comparison Matrix

| Concern                    | CDC (Debezium)          | Managed CDC      | Batch Export             | Reverse ETL       | Orchestrator Pipeline | S3 Shuttle               |
| -------------------------- | ----------------------- | ---------------- | ------------------------ | ----------------- | --------------------- | ------------------------ |
| **Direction**              | PG → SF                 | PG → SF          | PG → SF                  | SF → PG           | Either                | Both                     |
| **Latency**                | 1–5 min                 | 1–5 min          | Hours                    | Minutes–hours     | Minutes–hours         | Minutes–hours            |
| **Captures deletes**       | Yes                     | Yes              | No (without soft delete) | Yes (via diff)    | Manual                | No (without soft delete) |
| **Operational complexity** | High                    | Low              | Low                      | Low               | Medium                | Medium                   |
| **Cost**                   | Infrastructure          | Per-row SaaS     | Minimal                  | SaaS subscription | Infrastructure        | Infrastructure           |
| **Schema evolution**       | Configurable            | Automatic        | Manual                   | Automatic         | Manual                | Manual                   |
| **Data lake side effect**  | No (without extra work) | No               | Possible                 | No                | No                    | Yes (free)               |
| **Inspectability**         | Event logs              | Vendor dashboard | Files on disk/S3         | Vendor dashboard  | Application logs      | Parquet files in S3      |
| **Replayability**          | From Kafka offset       | Vendor manages   | Re-export                | Re-sync           | Re-run task           | Re-process S3 files      |
| **Vendor lock-in**         | Low (OSS)               | High             | None                     | High              | None                  | None                     |
| **Bidirectional**          | No                      | No               | No                       | No                | Yes (with effort)     | Yes (natively)           |

---

## Recommended Architecture

For the ML application architecture discussed previously, the cleanest setup is a two-loop pattern with the S3 shuttle as the backbone:

### Loop 1: Operational → Analytical (PostgreSQL → Snowflake)

**For high-frequency, change-sensitive tables** (user events, inference logs, orders): Use CDC via Debezium → NATS JetStream → S3 → Snowpipe. This captures every mutation in near-real-time and is worth the operational overhead for tables where freshness and completeness matter.

**For everything else** (reference data, config, dimension tables): Use the S3 shuttle on a schedule. The shuttle queries PostgreSQL (or a read replica), writes Parquet to S3, and Snowpipe ingests into Snowflake. Hourly or daily cadence. Simple, cheap, and the Parquet files in S3 give you a queryable archive.

If you want to skip CDC complexity entirely in the early stages, the S3 shuttle with timestamp-based watermarks can handle most tables at 5–15 minute freshness. Migrate the highest-frequency tables to CDC later when the need is proven.

### Loop 2: Analytical → Operational (Snowflake → PostgreSQL / Redis)

Use the S3 shuttle, triggered by the Dagster orchestrator after dbt models complete:

1. dbt runs feature engineering models in Snowflake.
2. Dagster triggers the shuttle service.
3. Shuttle queries Snowflake for the feature outputs, writes Parquet to S3.
4. Shuttle reads the Parquet and bulk-loads into Redis (online feature store) and PostgreSQL (for application-visible aggregates).

This gives you a single, consistent mechanism for both directions, with S3 as the durable, inspectable middle layer.

### The S3 Data Lake as a Bonus

Because every sync operation produces Parquet files in S3 with a predictable structure, you've accidentally built a data lake. This is genuinely valuable:

- **DuckDB** can query these files directly for ad-hoc analysis without touching either database.
- **Athena / Presto** can provide a SQL interface over the S3 data for teams that don't have Snowflake access.
- **ML training pipelines** can read directly from S3 Parquet rather than querying Snowflake, reducing warehouse compute costs.
- **Disaster recovery** — if either database has issues, S3 contains a complete history of every sync batch.
- **Regulatory compliance** — immutable Parquet files with timestamps provide an audit trail of data state at any point in time.

This is the S3 shuttle's strongest argument compared to direct-connection patterns: the intermediate storage layer isn't just a transport mechanism, it's a durable, queryable asset that adds value beyond the sync itself.