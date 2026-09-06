---
title: "0002 — splitting warehouse into redhouse + snowhouse"
description: Why the dual-warehouse app was split into two single-purpose apps.
sidebar:
  label: "0002 · warehouse split"
  order: 2
---

**Status:** accepted (2026-07-10) · implemented

## Context

`warehouse` served **both** Amazon Redshift and Snowflake from one binary and one
frontend. The Rust crate compiled both connectors (`default = ["postgres",
"snowflake"]`) and the React app fetched `/api/warehouse-type` at boot to decide
whether to render the Redshift routes or the Snowflake ones.

Keeping both behind one runtime switch made for a compromised experience on each
side: the binary shipped a connector it would never use, the frontend carried a
loading state and a discrimination context that existed only to pick a warehouse,
and every navigation/label decision branched on a runtime type that is in fact
fixed per deployment. The two warehouses also diverge — Redshift is performance-
and scan-oriented, Snowflake is cost- and credit-oriented — so the shared shell
was a lowest-common-denominator.

## Decision

Replace `warehouse` with two autonomous, single-purpose apps:

- **`redhouse`** — Redshift only (`default = ["postgres"]`).
- **`snowhouse`** — Snowflake only (`default = ["snowflake"]`).

Each is a fork of `warehouse` trimmed to one warehouse. The warehouse identity is
now fixed at **compile time**: each app knows its warehouse statically, which
removes the `/api/warehouse-type` boot fetch, the `WarehouseContext`, and all the
runtime branching in the dashboard shell. `warehouse` is deleted.

The generic frontend (design system, layout, table, charts, themes, lib helpers)
is **duplicated** into each app rather than extracted into a shared package —
matching the repo's existing convention: there is no JS workspace, and the
`data_view` and `warehouse` frontends were already standalone.

## Alternatives considered

- **Keep the single dual-mode `warehouse` app.** Rejected: the runtime switch is
  the source of the compromised experience, and each deploy only ever runs one
  warehouse anyway.
- **Split the backends but keep one shared frontend package.** Rejected: the
  monorepo has no bun/JS workspace, a strict-allowlist `.dockerignore`, and a
  frontend build wired through each app's Rust CLI. A shared package would mean
  new monorepo infrastructure (workspace root, `packages/` dir, Dockerfile and
  ignore changes) that recouples the two apps and their Docker builds — the
  opposite of the separation we want. Duplication is the established convention.

## Consequences

- Each app ships only the connector it uses; smaller, clearer binaries.
- The frontend loses its loading state and warehouse-discrimination context; the
  router and dashboard are built statically from one warehouse's routes.
- Each warehouse's UI can now diverge freely (Redshift performance vs Snowflake
  cost) without a shared shell holding it back.
- Cost: the generic UI now lives in two copies. Genuinely shared improvements must
  be applied to both — an accepted trade for full app independence.
- Both apps default to port `8050`; run one at a time locally, or override the
  port when running them side by side.
