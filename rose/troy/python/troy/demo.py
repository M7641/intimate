"""Run the two JSON boundaries end to end. `uv run python -m troy.demo`."""

from __future__ import annotations

import json

from pydantic import ValidationError

from troy import SAMPLES
from troy.ingest import validate_ingest
from troy.marts import reconcile_catalog, validate_rows
from troy.models import IngestCustomer


def _load(name: str):
    return json.loads((SAMPLES / name).read_text())


def _rule(title: str) -> None:
    print(f"\n\033[1m{title}\033[0m")
    print("─" * len(title))


def ingestion_boundary() -> None:
    _rule("1. INGESTION BOUNDARY  (OpenAPI / JSON Schema)")

    good = _load("ingest_good.json")
    errs = validate_ingest(good)
    print(f"  good payload -> {'ACCEPTED ✅' if not errs else errs}")
    # past the wall, work with a typed object
    customer = IngestCustomer.model_validate(good)
    print(f"  typed object -> {customer.customer_id} / {customer.tier.value}")

    bad = _load("ingest_bad.json")
    errs = validate_ingest(bad)
    print(f"  bad payload  -> REJECTED ❌ ({len(errs)} violations):")
    for e in errs:
        print(f"       • {e}")
    try:
        IngestCustomer.model_validate(bad)
    except ValidationError as exc:
        print(f"  pydantic also rejects it ({len(exc.errors())} errors) — defense in depth")


def marts_boundary() -> None:
    _rule("2. dbt MART BOUNDARY  (JSON Schema)")

    drift = reconcile_catalog(SAMPLES / "dbt_catalog.json")
    print(f"  catalog reconciliation -> {'in sync ✅' if not drift else drift}")

    rows = _load("dbt_rows.json")
    problems = validate_rows(rows)
    clean = {i for i in range(len(rows))} - {i for i, _ in problems}
    print(f"  row value invariants   -> {len(clean)}/{len(rows)} rows valid")
    for i, msg in problems:
        print(f"       • row[{i}] {msg}")


def main() -> None:
    print("\033[1m\nTROY — contract registry runtime demo\033[0m")
    print("the registry (../registry) is the source of truth; this only enforces it")
    ingestion_boundary()
    marts_boundary()
    print()


if __name__ == "__main__":
    main()
