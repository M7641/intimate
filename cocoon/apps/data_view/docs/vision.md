# Data View — Feature Vision

> **Goal of the application:** give a full, holistic view of the data in a
> database, then enable a myriad of dropdowns to explore it.

This document captures proposed features, organised from the most pragmatic to
the most idealistic.

---

## Where the app stands today

A **snapshot explorer** with three levels — `schema → table → timestamp` — and
three pages:

- **Data Explorer** — browse/filter rows, client-side search, CSV export.
- **Column Analysis** — per-column stats (null %, distinct, min/max/mean/median/
  quartiles), histograms, and comparison of up to 4 snapshots.
- **Table Info** — column metadata + Redshift/Snowflake storage details + row-
  count history.

The foundations are healthy:

- Tables are **event-sourced**: every row carries a `load_timestamp`, and the
  backend already exposes `timestamps`, `row_counts`, and a compare mode.
- Routes are split into **light** (metadata) and **heavy** (data scans) with
  separate timeouts.
- SQL safety rests on **regex validation of identifiers** + **value escaping**.

The main gap versus the stated goal: the app is **table- and column-centric, one
at a time**. It never shows the _whole database_, the _relationships_ between
tables, or _change over time_ beyond a row-count chart.

---

## Build status

A running log of what has shipped, so this document stays honest about the gap.

- ✅ **Catalogue map** (Angle A) — `/catalogue` page with a Cytoscape graph.
- ✅ **Schema health dashboard** (Angle A) — `/schema-health` page.

Everything below without a ✅ is still proposed, not built.

---

## Angle A — The "whole database" view that's missing

Today we only ever see the trees, never the forest.

- ✅ **Catalogue map (relationship graph)** — a landing screen showing all
  tables as a graph with inferred links. _Built:_ links come from two sources —
  **declared foreign keys** (Redshift and Snowflake let you define FKs even
  though they aren't enforced; these give direction and exact column pairs) and,
  where none are declared, **naming convention** (`customer_id` appearing in two
  tables). A `id`-bare column and any column spanning >12 tables are excluded to
  avoid a hairball. Clicking a link runs an on-demand **value-overlap** check on
  each side's latest snapshot, turning the naming guess into a measured
  coverage % — so the original "value overlap" idea became a per-edge validation
  rather than the primary inference. A living ERD that nobody maintains by hand.
- ✅ **Schema health dashboard** — a grid of every table showing: freshness of
  the last load and volume trend (↗︎↘︎ vs the previous snapshot), defaulting to
  most-stale-first so problems surface at the top. Empty tables are shown
  explicitly. _Not yet:_ **null spikes** and **drift alerts** — those are
  column-level and cross-snapshot (expensive), and overlap with Angle C's drift
  detection; deferred to a later layer.
- **Data dictionary** — an editable glossary layer (table description, business
  meaning per column, owner), persisted separately. Snowflake already returns
  `column_comment`; extend it into a home-grown annotation layer.

## Angle B — The "myriad of dropdowns": from filtering to faceting

The current filter model (`equals` / `contains`, `AND` only, 4 LIKE operators)
is the clearest gap versus the vision. Propose a conceptual jump: **from
filtering to faceted search** (the e-commerce model).

- **Facets with cross-counts** — each low-cardinality column becomes a facet
  listing its values _with the row count for each_, and those counts
  **recompute as you filter** on other columns. This is the "myriad of
  dropdowns" experience: you feel the shape of the data as you click.
- **Rich operators** — numeric ranges (`>=`, `between`), date pickers, `IN`
  multi-select, `IS NULL` / `IS NOT NULL`, `NOT`, and **OR groups**. A natural
  extension of `build_filter_clause`.
- **Saved & shareable views** — a filter + sort + columns encoded in the URL
  (deep-linking already exists partially via TanStack Router) and nameable.
  "Send me the link to this view" becomes trivial.
- **Server-side pagination + sorting** — today everything is client-side over
  1000 rows. Real exploration needs `OFFSET` / `ORDER BY` on the backend (the
  frontend already sorts locally, so the UI is ready).

## Angle C — Time travel (the hidden superpower)

The data is snapshotted. That is rare and valuable, and barely exploited.

- **Snapshot diff** — _which rows were added / removed / changed between two
  loads?_ For supply/demand/reference data this is often **the** question.
  Technically: compare by business key across two `load_timestamp` values (the
  repo's `scd4-history` skill encodes exactly this versioning logic).
- **Drift detection** — automatically compare a column's distribution between
  successive snapshots and flag abnormal moves (new categorical value, null
  explosion, mean shift). The existing compare mode already does half the work
  — make it _proactive_ instead of manual.
- **Row history** — pick a business key and see its values evolve over time
  (SCD-style timeline). The natural complement to the diff.
- **Animated time slider** — scrub through snapshots and watch the histogram
  "breathe". Striking and revealing for spotting a break.

## Angle D — Automated profiling & quality

- **One-click profiling report** per table (think `ydata-profiling`): for each
  column, inferred type, detected pattern (email, code, date), top values,
  outliers, completeness. The backend already computes null/distinct/quartiles
  — it's the aggregation and presentation that need industrialising.
- **Quality badges** — pills directly in the column list: "40% nulls",
  "cardinality = 1 (constant column?)", "inconsistent format".
- **Column-relationship discovery** — correlation matrix, scatter plots, and
  functional-dependency detection (`code` determines `label`). Today there is
  zero multi-column analysis.

---

## The idealistic / extreme tier: semantic & AI layer

- **Natural-language querying** → translated into the filter model (not raw SQL
  generated by an LLM, but a mapping onto our _already-validated_ operators, so
  the existing regex/escaping safety holds). "Show me supply rows where quantity
  dropped more than 50% since last week."
- **Auto-generated per-table narrative** — a paragraph at the top of Table Info:
  _"This table grew 12% over 7 days; column `region` gained a new value 'APAC';
  nulls in `price` went from 2% to 18% — likely anomaly."_ The diff + drift from
  Angle C feed this text directly.
- **Semantic catalogue search** — embeddings over column/table descriptions to
  "find tables about customer revenue" even without an exact name match. (The
  repo already has an `oracle` pilot around `rig`/embeddings — possible synergy.)
- **Proactive alerts** — drift no longer just displays; it _pushes_ a
  notification when a snapshot crosses a threshold.

---

## Ideas gathered while building

Things that surfaced during implementation and are worth keeping in view:

- **Declared foreign keys are a free, high-confidence signal.** Both warehouses
  store FK definitions even when they don't enforce them. Reading them turned
  the catalogue map from "guess by name" into "show what was declared, guess the
  rest" — a strict improvement that also gives edge direction and cross-name
  column pairs (`order.cust_ref → customer.id`).
- **Overlap as edge weight.** The per-edge coverage % is computed on demand
  today. It could instead thicken/colour edges automatically, so the graph shows
  relationship _strength_ at a glance — at the cost of scanning every visible
  edge (wants the metadata cache first).
- **Static (non-snapshot) tables are currently invisible** on the health
  dashboard, which only lists event-sourced tables. A "reference data" section
  with plain row counts would complete the "whole database" picture.
- **The health dashboard scans every table on each load.** It is honest but
  slow; it is the clearest customer for the metadata cache + async profiling
  below.

## Cross-cutting enablers (the unglamorous prerequisites)

These are not user-facing features but make most of the above feasible at scale.

- **Approximate statistics** — Redshift is a PostgreSQL 8.0.2 fork (no
  `WIDTH_BUCKET`, no modern functions). Exact distinct-counts are expensive;
  `APPROXIMATE COUNT(DISTINCT ...)` or reservoir sampling makes profiling large
  tables nearly free.
- **Metadata cache layer** — today every request hits the warehouse. Schemas and
  types rarely change; a TTL cache transforms UI responsiveness and makes
  cross-faceting possible without hammering Redshift.
- **Async profiling jobs** — heavy profiling runs as a background job with a
  cached result, rather than blocking a request.
- **Chart interactivity** — click a bar to filter; the charts are
  static today.

---

## Suggested starting order

If the objective is to maximise the sense of a holistic view for reasonable
effort:

1. **Snapshot diff** (Angle C) — strong differentiator, data already available,
   answers a real business question.
2. **Faceted filters with cross-counts** (Angle B) — directly realises the
   "myriad of dropdowns".
3. **Schema health dashboard** (Angle A) — finally delivers the "whole database"
   view.
4. _Then_ the cache layer + approximate stats as the performance foundation for
   extending everything else.

The AI tier comes after the diff and drift exist, because they are its raw
material.
