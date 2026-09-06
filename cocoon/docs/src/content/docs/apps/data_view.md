---
title: data_view
description: Snapshot data explorer over the warehouse, with a React UI.
sidebar:
  order: 2
---

**Snapshot data explorer** for supply / demand / reference data read from
Redshift, with a React frontend. Runs on port 8050. Built on `tako-database`,
`service-kit`, and `ouroboros`; production, Rust-only.

:::caution
data_view is **customer-facing**. Its responses should not leak the identity of
the backend or warehouse behind it.
:::

## Source

- Backend: `cocoon/apps/data_view`
- Frontend: `cocoon/apps/data_view/frontend` (React + Vite + bun)

This page is a stub — grow it as decisions accrue.
