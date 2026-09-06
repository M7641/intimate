"""extract — schema-driven product feature extraction (text + image)."""

from extract.domains import SCHEMAS, get_schema
from extract.extractor import VisionLanguageExtractor
from extract.pipeline import extract_features
from extract.schema import ExtractionField, ExtractionSchema, FieldKind

__all__ = [
    "SCHEMAS",
    "ExtractionField",
    "ExtractionSchema",
    "FieldKind",
    "VisionLanguageExtractor",
    "extract_features",
    "get_schema",
]
