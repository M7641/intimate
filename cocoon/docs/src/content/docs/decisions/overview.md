---
title: The decision log
description: How and why we record decisions over time.
sidebar:
  order: 0
---

This is where we record **why** choices were made — the reasoning that source
code and commit messages lose. Each entry is a numbered, dated record in the
spirit of an [Architecture Decision Record (ADR)](https://adr.github.io/).

## Why keep it

A decision is cheap to make and expensive to reconstruct. Six months on, the
question is rarely "what did we build" (the code answers that) but "why this and
not the obvious alternative". A record captures the constraint that tipped the
balance and what we knowingly gave up, so a future change is a deliberate
revision rather than an accidental reversal.

## Format

Each record is one Markdown file, `NNNN-short-title.md`, and follows the same
sections:

- **Status** — proposed · accepted · superseded (link the successor).
- **Context** — the forces at play; the problem, not the solution.
- **Decision** — what we chose, stated plainly.
- **Alternatives considered** — the options we rejected, and why.
- **Consequences** — what becomes easier, what becomes harder.

## Conventions

- Records are **append-only**. To change a decision, write a new record and mark
  the old one *superseded* — never rewrite history.
- Number sequentially; the number never changes once assigned.
- Keep one decision per record. If you are deciding two things, write two.

## Records

- [0001 — tako: editing uploaded tables via satellites + a metadata registry](/decisions/0001-tako-satellite-registry/)
- [0002 — splitting warehouse into redhouse + snowhouse](/decisions/0002-warehouse-split/)
- [0003 — snowhouse owns a query-history cache table](/decisions/0003-snowhouse-query-history-cache/)
- [0004 — snowhouse analytics read from the cache, not information_schema](/decisions/0004-snowhouse-analytics-read-from-cache/)
