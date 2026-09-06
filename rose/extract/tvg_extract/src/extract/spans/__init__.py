"""A4 span extraction — see :mod:`extract.spans.spans`."""

from extract.spans.spans import (
    GLiNERLocator,
    entity_labels,
    locate_frame,
    spans_to_record,
)

__all__ = [
    "GLiNERLocator",
    "entity_labels",
    "locate_frame",
    "spans_to_record",
]
