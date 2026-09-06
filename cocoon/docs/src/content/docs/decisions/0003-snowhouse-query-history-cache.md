---
title: "0003 — snowhouse owns a query-history cache table"
description: Why snowhouse persists its own query history to bypass the 10k-row information_schema cap.
sidebar:
  label: "0003 · query-history cache"
  order: 3
---

**Status:** accepted (2026-07-14) · implemented (Phase 1)

## Context

Every snowhouse analytics page reads from the
`INFORMATION_SCHEMA.QUERY_HISTORY` table function. That function has two limits
that only bite at scale:

- **`RESULT_LIMIT` maxes out at 10,000 rows per call**, and the function has
  **no `OFFSET`** — so there is no way to page through a busy week by index.
- Its retention is ~7 days.

A single busy day can exceed 10,000 queries, so "analyse a full week" is simply
not answerable in one call, and any fixed time-slice risks silently returning a
truncated 10,000 rows with no signal that more existed.

The clean fix — `SNOWFLAKE.ACCOUNT_USAGE.QUERY_HISTORY` (no row cap, 365-day
retention) — requires `IMPORTED PRIVILEGES` on the `SNOWFLAKE` database,
typically account-admin. We do not have that grant, and cannot assume it.

## Decision

Snowhouse **owns a persistent table** it fills itself
(`SNOWHOUSE_CACHE_TABLE`, fully-qualified) and then queries without the 10k cap.
The app defines the DDL, creates it, backfills it, and keeps it current, all
through a new `cache` CLI verb group (`init` / `backfill` / `refresh` /
`status`). The table is an **append-only event log** keyed logically on
`query_id` — not an SCD history table; a completed query is immutable.

Two design choices make it correct:

- **Adaptive time-windowing.** The table function is paged by `end_time` range,
  never by offset. Each window's `COUNT(*)` is probed; a window that reaches the
  10k cap is bisected in time until every slice fits underneath it. This is what
  guarantees no silent truncation — the failure mode a fixed window would hide.
- **Idempotent upsert.** Rows land via `MERGE … WHEN NOT MATCHED` on
  `query_id`. Overlapping windows, re-runs, and the refresh overlap are all
  therefore safe. `backfill` (a 7-day sweep) and `refresh` (everything since the
  `MAX(end_time)` watermark, re-scanned from one hour before it) share this one
  upsert primitive.

The write path is a new `AppState::blocking_execute` that runs DML/DDL and
deliberately bypasses the read-through query cache; the watermark and count
probes use a matching cache-bypassing read. Scheduling is offered two ways: an
external `cache refresh` job (recommended), or an in-process hourly ticker in
`serve` behind `SNOWHOUSE_CACHE_AUTO_REFRESH`.

## Alternatives considered

- **Require the `ACCOUNT_USAGE` grant.** Rejected as a hard dependency: we
  cannot assume account-admin will grant it, and `account_usage` lags reality by
  up to ~45 minutes whereas `information_schema` is real-time. If the grant does
  land later, the cache collapses to a plain `SELECT` — this decision is not a
  barrier to that.
- **Fixed-size time windows (e.g. one hour each).** Rejected: query volume is
  uneven, so a busy hour silently truncates at 10k while a quiet hour wastes a
  call. Saturation-driven bisection is the only variant that cannot drop rows.
- **Pull rows into Rust and re-insert.** Rejected: the `MERGE` reads the table
  function directly as its source, so ingestion stays entirely server-side — no
  row marshalling, no parameter limits.
- **In-process ticker only.** Rejected as the sole mechanism: multiple replicas
  would each write (correct via the `MERGE`, but wasteful), so an external
  single-writer job is the recommended default and the ticker is opt-in.

## Consequences

- Snowhouse now **writes** to Snowflake, not just reads. It needs a table it has
  `CREATE TABLE` on, named via `SNOWHOUSE_CACHE_TABLE`.
- A full week (and, over time, more than the 7-day `information_schema` horizon)
  becomes queryable in one shot.
- The cache inherits the connecting **role's** visibility — it holds exactly the
  queries `QUERY_HISTORY` returns for that role, no more.
- Freshness depends on the refresh cadence (default hourly); the cache is
  eventually-consistent with live query history, not real-time.
- Phase 2 (decided separately in
  [0004](/decisions/0004-snowhouse-analytics-read-from-cache/)): repoint the
  analytics endpoints at the cache table. Note that 0004 **drops** the
  `information_schema` fallback floated here — the cache became the single
  history source, enforced at startup.
