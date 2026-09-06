# SCD Type 4 — production edge cases

Read this before trusting an SCD4 load for audit. The four-step load in `SKILL.md`
is the happy path; these are the cases that quietly corrupt history if ignored.

## Contents

1. [Deletes: hard vs soft](#deletes-hard-vs-soft)
2. [Late-arriving and out-of-order data](#late-arriving-and-out-of-order-data)
3. [Schema evolution: adding a tracked column](#schema-evolution-adding-a-tracked-column)
4. [Self-contained vs superseded-only history](#self-contained-vs-superseded-only-history)
5. [Idempotent re-runs](#idempotent-re-runs)
6. [Validation queries](#validation-queries)
7. [Performance on Redshift](#performance-on-redshift)

---

## Deletes: hard vs soft

A business key can vanish from the source. Three policies — pick deliberately:

- **Ignore** (default, delta feeds). Staging carries only changed rows; absence
  means "no news", not "deleted". Do nothing. This is the generator's default mode.
- **Hard delete** (full-snapshot feeds). Absence means the key is gone. Run the
  generator with `--full-snapshot`: it closes the open history row
  (`valid_to = {load_ts}`, `is_current = FALSE`) and removes the row from the
  current table. History still shows the key existed up to `{load_ts}`.
- **Soft delete.** Some sources send a `deleted_at` / `is_deleted` flag instead of
  dropping the row. Treat that flag as a *tracked column* so the deletion becomes
  just another version. The current table keeps the row with the flag set; history
  records when it flipped. Often preferable to hard delete because the current
  table stays queryable ("show me churned customers").

Never mix modes in one feed. A delta feed run with `--full-snapshot` retires every
key not in the current batch — a silent mass deletion.

## Late-arriving and out-of-order data

The four-step load assumes each batch is *newer* than what's in history. If an
event arrives late (its real effective time is before the current open version),
the naive load stamps `valid_from = {load_ts}` and creates an out-of-order seam.

Two robust options:

- **Carry an event timestamp.** If staging has the real `effective_at`, use it for
  `valid_from` instead of `{load_ts}`, and on close set the *prior* row's
  `valid_to = effective_at`. Out-of-order then requires re-sequencing the affected
  key's versions (close the row whose window contains `effective_at`, split it).
  This is a per-key correction, not a bulk step — isolate the late keys and fix
  them in their own statement.
- **Reprocess the key.** For low volumes, delete all history rows for the affected
  business key and replay its events in order. Simpler, correct, and fine when late
  arrivals are rare.

If your source can deliver events out of order, decide this up front — retrofitting
effective-time onto a `{load_ts}`-based history is painful.

## Schema evolution: adding a tracked column

You add a column to `tracked_columns` after history already exists. Two problems:

1. **Old history rows lack the column.** `ALTER TABLE ... ADD COLUMN` backfills
   NULL. Decide whether NULL is acceptable or whether you backfill from a source.
2. **The hash changes.** Every existing key's stored `row_hash` was computed over
   the old column set. The next load recomputes over the new set, so **every key
   looks changed** and you get a spurious version for all of them.

Mitigations:

- Accept the one-time mass version (clean, honest: "the tracked attribute set
  changed on this date") — usually the right call.
- Or recompute `row_hash` for all current history rows with the new expression in
  the same migration, so the next load sees no spurious diff. Regenerate the load
  SQL first (it embeds the new hash expression), then run its hash expression as an
  `UPDATE ... SET row_hash = <new expr> WHERE is_current`.

Adding a `static_column` is harmless — it isn't hashed.

## Self-contained vs superseded-only history

This skill's history table is **self-contained**: it holds the current version too
(`is_current = TRUE`). The classic textbook Type 4 keeps history = *superseded
rows only*, with the current row living solely in the main table.

| | Self-contained (this skill) | Superseded-only (textbook) |
|---|---|---|
| Point-in-time query | history alone | history `UNION` current table |
| Storage | +1 row per key | minimal |
| "Current" duplicated | yes | no |

Self-contained wins for analytics: every time-travel query hits one table and the
`is_current` flag is a clean filter. If storage is genuinely tight and you never
time-travel across "now", switch to superseded-only by dropping step 3's
`is_current = TRUE` rows for the current version — but that complicates every
point-in-time query forever. Default to self-contained.

## Idempotent re-runs

Re-running the same batch must be a no-op, or backfills and retries corrupt
history. The generated load is **naturally idempotent on a delta** because step 1's
hash diff finds nothing changed the second time — `_scd_changed` is empty, steps
2-4 touch zero rows.

It is **not** idempotent if `{load_ts}` differs between runs *and* the data also
differs, which is the normal forward case. The thing to guarantee: one logical
batch = one `{load_ts}`. If a run fails mid-transaction, the `BEGIN/COMMIT` rolls it
back whole, so retry with the *same* `{load_ts}` is safe. Never retry a failed load
with a fresh timestamp.

## Validation queries

Run these after a load (or on a schedule) to catch a broken history. Each should
return **zero rows**.

```sql
-- A. More than one open version per key (close step missed a row)
SELECT customer_id, COUNT(*)
FROM analytics.dim_customer_history
WHERE is_current = TRUE
GROUP BY customer_id HAVING COUNT(*) > 1;

-- B. Overlapping windows for the same key (timestamp seam broken)
SELECT a.customer_id
FROM analytics.dim_customer_history a
JOIN analytics.dim_customer_history b
  ON a.customer_id = b.customer_id AND a.scd_id <> b.scd_id
WHERE a.valid_from < b.valid_to AND b.valid_from < a.valid_to;

-- C. Gaps between consecutive versions (a close used a different clock than the insert)
SELECT customer_id, valid_to AS gap_starts
FROM (
  SELECT customer_id, valid_to,
         LEAD(valid_from) OVER (PARTITION BY customer_id ORDER BY valid_from) AS next_from
  FROM analytics.dim_customer_history
)
WHERE next_from IS NOT NULL AND next_from <> valid_to;

-- D. Current table and history disagree on the live row
SELECT c.customer_id
FROM analytics.dim_customer c
LEFT JOIN analytics.dim_customer_history h
  ON c.customer_id = h.customer_id AND h.is_current = TRUE
WHERE h.customer_id IS NULL;
```

`A` and `D` catch a botched upsert; `B` and `C` catch a `{load_ts}` that wasn't a
single instant. Wire them as a post-load test (a dbt test, or a `SELECT` that
fails the job on non-zero count).

## Performance on Redshift

- **DISTKEY on the business key** (the generator sets this) co-locates a key's
  versions on one slice, so the close/insert joins stay local.
- **SORTKEY (business_key, valid_from)** makes point-in-time range scans and the
  "latest version" lookup cheap.
- **VACUUM / ANALYZE** the history table periodically — the close step is an
  `UPDATE`, which in Redshift is a delete + insert under the hood and leaves dead
  rows. A history table that's never vacuumed slowly bloats.
- For very large dimensions, stage the hash on the incoming data once
  (`stg_<table>` with a `row_hash` column) rather than recomputing it twice in
  step 1's `WHERE`. The generator inlines it for portability; precomputing is a
  worthwhile optimization at scale.
