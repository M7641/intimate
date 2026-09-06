---
title: tako
description: Batch data-ingest API for data warehouses.
sidebar:
  order: 1
---

**Batch data-ingest API for data warehouses.** Accepts CSV, JSON, Parquet, Avro,
and Vortex files, normalises every format to Parquet, uploads to S3, and loads
into the configured warehouse (DuckDB, Redshift, or Snowflake). Runs on port
3000.

## Shape

Table creation (DDL) and data ingestion are **separate concerns**: `POST /table`
creates the staging table, `POST /upload` only `COPY`s into it. This keeps each
request a single warehouse statement.

A schema opts into **Data Vault** raw-vault enrichment by declaring a
`business_key`: every uploaded row is enriched, insert-only, with a hub hash key,
a `hashdiff`, and load metadata. This is also how re-uploads are handled — append
and trace, never silently overwrite.

## Operational commands

- `tako serve` — run the ingest API (default with no subcommand).
- `tako innit-db` — initialise the warehouse: create the `sandpit` schema and the
  satellite registry tables (idempotent). See
  [decision 0001](/decisions/0001-tako-satellite-registry/).

## Source

- Code: `cocoon/apps/tako`
- Full reference: the app's own `README.md` (endpoints, env vars, file formats,
  deployment notes).
