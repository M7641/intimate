---
title: "0004 — snowhouse analytics read from the cache, not information_schema"
description: Why every analytics endpoint reads query history exclusively from the owned cache table, with no fallback.
sidebar:
  label: "0004 · analytics read the cache"
  order: 4
---

**Status:** accepted (2026-07-14) · implemented

## Context

[0003](/decisions/0003-snowhouse-query-history-cache/) gave snowhouse an owned
query-history cache table and the `cache` CLI to fill it, but left it
**write-only**: every analytics endpoint still read the live
`INFORMATION_SCHEMA.QUERY_HISTORY` table function, capped at 10,000 rows. The
cache delivered no value until something read it. 0003 sketched a Phase 2 that
would "keep the `information_schema` path as a fallback".

## Decision

All eleven query-history analytics endpoints now read **exclusively** from the
cache table. The table-function call is gone from every endpoint; each reads
`FROM <SNOWHOUSE_CACHE_TABLE>` instead.

We **dropped the fallback** floated in 0003. Carrying two history sources means
two code paths, two sets of behaviour to reason about, and a silent downgrade to
the 10k cap whenever the cache is unavailable — the exact failure 0003 exists to
remove. Instead the cache is the single source of truth, and its presence is a
hard precondition:

- **Fail at boot, not per request.** `serve` resolves `SNOWHOUSE_CACHE_TABLE`
  and probes it (`SELECT 1 … LIMIT 1`) before binding the port. Unset or missing
  → the process exits with a message pointing at `cache init`. A misconfigured
  deploy dies immediately and loudly, rather than serving truncated or empty
  analytics.
- **The window filter moved to `WHERE`.** The table function's
  `END_TIME_RANGE_START` did double duty — dodging the 10k cap *and* bounding the
  window. Reading a plain table, the cap is gone (the point) and the window
  becomes an ordinary `WHERE start_time >= …`. Most endpoints already had that
  filter; `credits-by-day` relied on the function argument alone and gained an
  explicit lower bound.
- **One new cached column.** `queue-analysis` and `warehouse-utilization` need
  `queued_provisioning_time`, so it was added to the cache DDL and the ingest
  column list. Existing rows predating the column read as `NULL` until re-ingested.

The read seam is a single `history_source()` helper (a process-wide `OnceLock`
set at startup), so no endpoint hard-codes the table name and unit tests still
build SQL without a live connection.

## Alternatives considered

- **Keep the `information_schema` fallback (0003's Phase 2 sketch).** Rejected:
  a fallback silently reintroduces the 10k truncation the cache removes, and
  doubles the behaviour to test. A hard dependency with a loud boot failure is
  simpler to operate and reason about.
- **Resolve the table lazily on first request.** Rejected: it defers a
  configuration error to a user-facing 500 instead of surfacing it at deploy
  time.
- **Thread the table name through every `query()` signature.** Rejected as
  churn: the name is fixed for the process lifetime, so a `OnceLock` set once at
  startup is both simpler and keeps the SQL builders unit-testable.

## Consequences

- Snowhouse **cannot serve without a populated cache**. Operating it now means
  running `cache init` + `backfill` before the first `serve`, and `cache refresh`
  (or the in-process ticker) to keep it current — see
  [0003](/decisions/0003-snowhouse-query-history-cache/) and the app README.
- Analytics can now span a full week (and, over time, beyond the 7-day
  `information_schema` horizon) without silent truncation.
- Freshness is bounded by the refresh cadence, not real time: the endpoints are
  now as fresh as the last `cache refresh`, not live to the second.
- `credits-by-day` stays disabled (too slow historically); it was repointed for
  correctness but not re-enabled here.
