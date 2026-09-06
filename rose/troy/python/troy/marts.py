"""dbt mart boundary: validate what dbt actually produced against the contract.

Two checks, mirroring the two things that can go wrong:

  1. STRUCTURE drift — reconcile the declared columns against dbt's catalog.json
     (did dbt build the columns/types we promised?). dbt's own model contracts
     also do this; we include it so the contract is the single reference.

  2. VALUE invariants — validate sample rows against the JSON Schema (ranges,
     enums, key format). This is what dbt contracts CANNOT express.
"""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from troy import REGISTRY


def _schema() -> dict:
    return json.loads((REGISTRY / "marts" / "dim_customer.schema.json").read_text())


def reconcile_catalog(catalog_path: Path) -> list[str]:
    """Compare declared columns/types against a dbt catalog.json. Returns drift."""
    schema = _schema()
    expected = schema["x-dbt"]["expected_columns"]
    model = schema["x-dbt"]["model"]

    catalog = json.loads(catalog_path.read_text())
    node = next(
        (n for n in catalog.get("nodes", {}).values()
         if n.get("metadata", {}).get("name") == model),
        None,
    )
    if node is None:
        return [f"model '{model}' not found in catalog"]

    actual = {name: col["type"] for name, col in node.get("columns", {}).items()}
    drift: list[str] = []
    for col, want_type in expected.items():
        if col not in actual:
            drift.append(f"missing column '{col}' (expected {want_type})")
        elif actual[col] != want_type:
            drift.append(f"column '{col}' type drift: catalog={actual[col]} expected={want_type}")
    for col in actual.keys() - expected.keys():
        drift.append(f"undeclared column '{col}' present in mart")
    return drift


def validate_rows(rows: list[dict]) -> list[tuple[int, str]]:
    """Validate each mart row against value invariants. Returns (row_index, error)."""
    validator = Draft202012Validator(_schema())
    problems: list[tuple[int, str]] = []
    for i, row in enumerate(rows):
        for e in validator.iter_errors(row):
            loc = "/".join(map(str, e.path)) or "<root>"
            problems.append((i, f"{loc}: {e.message}"))
    return problems
