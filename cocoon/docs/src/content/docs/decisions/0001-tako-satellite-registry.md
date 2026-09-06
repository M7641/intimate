---
title: "0001 — tako: editing uploaded tables via satellites"
description: How a table created by tako's upload route can be altered, and how we track it.
sidebar:
  label: "0001 · tako satellites"
  order: 1
---

**Status:** accepted (2026-06-23) · partially implemented

## Context

A table created through tako's `/upload` route is, by design, append-only and
immutable: `/table` only does `CREATE TABLE IF NOT EXISTS` and never alters a
live table. But users legitimately need to *change* such a table — add a new
attribute, or correct values for some rows.

The hard constraint is how the warehouse `COPY` matches columns:

- DuckDB and Redshift `COPY` are **position-matched** — widening a table and then
  loading files of different widths silently misaligns the data.
- Snowflake `COPY INTO … MATCH_BY_COLUMN_NAME` is **name-matched**.

So "just add a column" is not free; it pulls the loader toward name-matching
everywhere and a migration mechanism. tako's design is already Data-Vault shaped:
a schema that declares a `business_key` gets a hub hash key (`<entity>_hk`) on
every uploaded row.

## Decision

Allow a table to be "edited" by attaching **satellites** that share the original
table's business key. New or corrected attributes land in a new satellite table
keyed by the same `<parent>_hk`; an "edited" working view is assembled by joining
the hub to its satellites. This is Data Vault's native answer and needs no DDL on
live tables.

Track the model in a **metadata registry**, always in the `sandpit` schema:

- `sandpit.hub_registry` — identity of each hub (schema, name, business-key
  columns).
- `sandpit.satellite_registry` — the satellites per hub, with a `precedence`
  (for column-conflict resolution) and an `active` flag (soft delete).

The registry is created by the `tako innit-db` CLI command (idempotent). Limit
satellites to **10 per hub**; beyond that, raise a support ticket.

## Alternatives considered

- **A — Satellites (chosen).** Append-only, immutable, no live DDL, full history,
  parallel-load friendly. Cost: more tables and a join to reassemble a row.
- **B — Evolve the table (`ALTER TABLE ADD COLUMN`).** One wide table, familiar
  mental model. Rejected: mutates a live table and forces position-matched loads
  to switch to name-matched, plus a real migration step — giving up today's
  "one statement per request" simplicity.
- **C — Versioned table (`<table>_v2` + a union view).** Tables stay immutable.
  Rejected for now: table/version proliferation and union plumbing.
- **D — Semi-structured overflow column (`VARIANT`/`SUPER`/`JSON`).** Zero DDL,
  maximally flexible. Rejected: weak typing, uneven across backends, and it
  bypasses schema validation.

On the registry itself, we considered **deriving** it from the schema registry
plus `information_schema` instead of storing it. We chose a stored table because
we want satellites to be **API-registered** (not only file-declared); a stored
registry is the source of truth for that path, at the cost of having to keep it
in sync with the warehouse.

## Consequences

Easier:

- Editing a table needs no DDL on the original and keeps full history.
- The satellite scaffolding already exists in the `schema` crate
  (`DvTableType::Satellite`, `build_satellite_scaffolding`).

Harder / still open:

- **Enrichment is not yet wired for satellites.** `enrich_dataframe` keys off the
  legacy `business_key()` and ignores `DvTableType::Satellite`, so a satellite's
  `<parent>_hk` is created in the DDL but not populated on upload. The satellite
  file must carry the business-key columns to re-derive the hash. This is the
  next step.
- **The "edited view" is more than a left join.** Satellites are historised
  (`load_end_date`), so the view must pick the current row per satellite and
  resolve column conflicts by `precedence` — not a plain join.
- **Registry drift.** A stored registry must be written atomically with satellite
  creation (one transaction) and reconciled against the warehouse, or it diverges.
- No tests yet cover the DV2.0 path.
