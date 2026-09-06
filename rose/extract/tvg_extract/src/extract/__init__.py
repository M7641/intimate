"""extract — schema-driven product feature extraction from text."""

from extract.domains import SCHEMAS, get_schema
from extract.extractor import TextExtractor
from extract.pipeline import extract_features
from extract.schema import ExtractionField, ExtractionSchema, FieldKind

__all__ = [
    "SCHEMAS",
    "ExtractionField",
    "ExtractionSchema",
    "FieldKind",
    "TextExtractor",
    "extract_features",
    "get_schema",
]
