# Troy — a multi-language contract registry

The walled city that succeeds `wall`. Where `wall` explored CUE, Troy takes the
decision from that review: **CUE's runtime is Go-only, so a Rust+Python world
needs contract formats that are born multi-language.** Troy uses two —
**Protobuf** and **OpenAPI/JSON Schema** — each at the boundary where it wins.

## The one rule

> **Binary inside, text at the edge.**

```
  EXTERIOR  (untrusted, human-readable JSON)
     clients ──JSON──▶ [ingestion API]            OpenAPI / JSON Schema
                            │  validated
  ──────────────────────────┼───────────────────────────────────────────
  INTERIOR  (trusted, fast, controlled)           Protobuf / gRPC
     [Axum Rust] ──binary──▶ [Python service]
  ──────────────────────────┼───────────────────────────────────────────
  DATA AT REST                ▼
     dbt marts ──validated against──▶ JSON Schema
```

## Layout

```
registry/                     ← THE SOURCE OF TRUTH. All contracts live here.
  proto/                        internal Rust↔Python contract (Protobuf)
    contracts/customer/v1/customer.proto
    buf.yaml, buf.gen.yaml      lint / breaking-change / codegen config
  openapi/ingest.yaml           ingestion boundary (OpenAPI 3.1)
  marts/dim_customer.schema.json  dbt mart contract (JSON Schema 2020-12)

python/                       ← consumes the registry at the JSON boundaries
  troy/ingest.py                validate untrusted payloads (OpenAPI schema)
  troy/marts.py                 reconcile dbt catalog + validate mart rows
  troy/models.py                pydantic "generated types" (ergonomic layer)
  troy/demo.py                  runnable: good + bad cases for both boundaries
  samples/                      example payloads / dbt artifacts

rust/                         ← the internal binary boundary
  build.rs                      compiles the .proto with `protox` (no protoc)
  src/main.rs                   typed Customer, binary wire, validation
```

The two consumer trees **never define schemas** — they load them from
`registry/`. That is the registry discipline: schema-first, one source of truth.

## Run it

```bash
make demo          # both boundaries
make demo-python   # ingestion + dbt marts   (needs uv)
make demo-rust     # internal Protobuf        (needs cargo)
```

Expected: the ingestion boundary accepts a clean payload and rejects a bad one
with 5 precise violations; the dbt boundary reconciles the catalog and flags the
one invalid row; the Rust boundary shows a 38-byte protobuf message vs 97 bytes
of JSON and rejects a malformed customer.

## Why each format where it is

| Boundary                 | Format          | Decisive reason                                                        |
| ------------------------ | --------------- | ---------------------------------------------------------------------- |
| Rust ↔ Python (internal) | **Protobuf**    | binary wire, mature codegen both sides, `buf breaking`                 |
| Ingestion (external)     | **OpenAPI**     | native value constraints, readable JSON, universal clients             |
| dbt marts (data)         | **JSON Schema** | value invariants dbt contracts can't express; validates `catalog.json` |

Ingestion and marts share one engine — JSON Schema — so you learn one
constraint grammar and reuse it for live input _and_ data at rest.

## Production vs this prototype

| Concern                | Prototype (runs here)                     | Production path                                         |
| ---------------------- | ----------------------------------------- | ------------------------------------------------------- |
| Proto → Rust           | `protox` in `build.rs` (no system protoc) | `buf generate` (also emits the Python betterproto side) |
| Proto value rules      | mirrored in Rust `validate()`             | `buf.validate` annotations + `protovalidate` runtime — see [docs/protovalidate.md](docs/protovalidate.md) |
| OpenAPI → Python types | hand-written `models.py`                  | `datamodel-code-generator`                              |
| Breaking changes       | —                                         | `buf breaking` (Protobuf), `oasdiff` (OpenAPI) in CI    |

## Versioning — the governed-registry piece (implemented)

What turns a folder of schemas into a registry: every PR is gated against the
base branch, and a backward-**incompatible** contract change fails CI.

| Surface | Tool |
|---------|------|
| Protobuf | `buf breaking` |
| OpenAPI | `oasdiff breaking` |
| Marts (JSON Schema) | `troy.compat` — in-repo checker (no off-the-shelf tool exists) |

```bash
make compat-demo    # runnable now: the marts checker on bundled v1/v2 schemas
make check-compat   # full gate vs origin/main (skips buf/oasdiff if absent)
```

One script — `scripts/check-compat.sh` — drives all three, called by both the
Makefile and CI (`.github/workflows/troy-contracts.yml`), so local == CI.
Breaking changes are meant to become **new versions** (`…v2`), never silent
mutations of `v1`. Full policy + the marts breaking-change rules:
[docs/versioning.md](docs/versioning.md).
