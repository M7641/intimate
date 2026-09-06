---
title: Applications overview
description: The cocoon application family at a glance.
sidebar:
  order: 0
---

cocoon is a Cargo workspace. Inter-crate dependencies are inferred from
`Cargo.toml`; the canonical project graph lives in `.moon/workspace.yml`.

| Application | Role | Port | Status |
| --- | --- | --- | --- |
| [tako](/apps/tako/) | Data ingest API: CSV/JSON/Parquet/Avro/Vortex → Parquet → S3 → warehouse | 3000 | production |
| [data_view](/apps/data_view/) | Snapshot data explorer (supply/demand/reference from Redshift) + React UI | 8050 | production |
| [redhouse](/apps/redhouse/) | Amazon Redshift analytics & cost explorer | 8050 | production |
| [snowhouse](/apps/snowhouse/) | Snowflake analytics & cost explorer | 8050 | production |
| [nyx](/apps/nyx/) | LLM-powered dev CLI agents (commits, clarity, Q&A) | — | prototype |
| [augur](/apps/augur/) | Local completion server for Zed edit prediction (GGUF on Metal) | — | prototype |

These apps share the `service-kit` (Axum scaffolding), `database`,
`blobs`, `schema`, and `ouroboros` crates. Each page below is a stub to grow
over time — the source `README.md` remains the source of truth until a page
says otherwise.
