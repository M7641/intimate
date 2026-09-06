"""Ingestion boundary: validate untrusted JSON against the OpenAPI schema.

The OpenAPI `components/schemas/IngestCustomer` block IS plain JSON Schema, so
we load it and validate directly — schema-first, no codegen. This is the
runtime enforcement CUE could not give us outside Go.
"""

from __future__ import annotations

import functools

import yaml
from jsonschema import Draft202012Validator, FormatChecker

from troy import REGISTRY


@functools.cache
def _ingest_schema() -> dict:
    spec = yaml.safe_load((REGISTRY / "openapi" / "ingest.yaml").read_text())
    return spec["components"]["schemas"]["IngestCustomer"]


@functools.cache
def _validator() -> Draft202012Validator:
    # format_checker enables `format: email` etc.; core keywords (pattern,
    # enum, minimum/maximum, additionalProperties) are always enforced.
    return Draft202012Validator(_ingest_schema(), format_checker=FormatChecker())


def validate_ingest(payload: dict) -> list[str]:
    """Return a list of human-readable validation errors ([] means valid)."""
    errors = sorted(_validator().iter_errors(payload), key=lambda e: list(e.path))
    return [f"{'/'.join(map(str, e.path)) or '<root>'}: {e.message}" for e in errors]
