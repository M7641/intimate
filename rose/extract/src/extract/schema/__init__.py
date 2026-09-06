"""The extraction abstraction — see :mod:`extract.schema.schema`."""

from extract.schema.schema import (
    ExtractionField,
    ExtractionSchema,
    FieldKind,
    load_schema,
)

__all__ = ["ExtractionField", "ExtractionSchema", "FieldKind", "load_schema"]
