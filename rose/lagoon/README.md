# lagoon

A pilot for using **[MiniStack](https://github.com/ministackorg/ministack)** to
spin up local AWS services in tests, then query them from Python. It stages data
in **S3** and loads/queries a **Postgres warehouse**, all against a single
local container.

> **Package:** MiniStack — <https://github.com/ministackorg/ministack>
> A free, MIT-licensed local AWS emulator: 56+ services on one port (4566),
> drop-in compatible with boto3 / the AWS CLI, ~270 MB image, <2 s startup.

## ⚠️ A note on Redshift

The original goal was to spin up **S3 and Redshift**. MiniStack **does not
emulate Amazon Redshift** — it is not among its 56+ supported services.

This pilot therefore uses MiniStack's **RDS PostgreSQL** as a Redshift stand-in.
That is a deliberate, reasonable substitution because **Redshift is
wire-compatible with PostgreSQL** (it began as a fork of PostgreSQL 8.0.2): the
same `psycopg` driver and the same `SELECT` statements used here also run
against a real Redshift cluster, so the query code ports across with only a
connection-string change.

What does **not** carry over (and is out of scope for this pilot):

- `COPY ... FROM 's3://...'` bulk loads (we `INSERT` instead)
- Redshift-only DDL: `DISTKEY`, `SORTKEY`, `ENCODE`, columnar storage
- Redshift system tables (`STL_*`, `SVL_*`) and `UNLOAD`

If you need true Redshift behaviour, point the warehouse half of the pipeline at
a real Redshift cluster and keep using MiniStack only for S3.

## Layout

```
src/lagoon/
  ministack.py   # start/stop/reset a MiniStack Docker container
  pipeline.py    # S3 staging + RDS-Postgres warehouse, plus the query examples
  cli.py         # end-to-end demo (`lagoon`)
tests/
  conftest.py    # session-scoped MiniStack fixture, reset between tests
  test_pipeline.py
```

## Prerequisites

- Docker (MiniStack runs as a container; its RDS support boots real Postgres
  containers, so the daemon must be reachable)
- [uv](https://docs.astral.sh/uv/)

## Run the demo

```bash
uv sync
uv run lagoon
```

Expected output (abridged):

```
Query S3 (staged objects):
  - events/1.json
  - events/2.json
  - events/3.json

Query warehouse (total amount per region):
  - amer: 80
  - emea: 170
```

## Run the tests

```bash
uv run pytest
```

The suite starts MiniStack **once** per session and calls
`POST /_ministack/reset` before each test for isolation. If Docker is not
available the tests are **skipped**, not failed.

## How it works

1. `MiniStack.start()` runs `docker run -d --rm -p 4566:4566 ministackorg/ministack`
   and polls `/_ministack/health` until it returns `200`.
2. **S3** is emulated natively — a standard boto3 client pointed at
   `http://localhost:4566` creates a bucket and stages objects.
3. The **warehouse** is created through the **RDS** API
   (`create_db_instance(Engine="postgres", …)`); MiniStack boots a real Postgres
   container and returns its endpoint, which `psycopg` then connects to.
4. Both stores are queried and the results compared.

## Reference

- MiniStack: <https://github.com/ministackorg/ministack>
