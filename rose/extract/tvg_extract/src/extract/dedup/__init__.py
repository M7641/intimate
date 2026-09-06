"""Step-0 dedup + cache — see :mod:`extract.dedup.dedup`."""

from extract.dedup.dedup import (
    ExtractionCache,
    description_key,
    merge_near_duplicates,
    near_duplicate_leaders,
    normalise,
)

__all__ = [
    "ExtractionCache",
    "description_key",
    "merge_near_duplicates",
    "near_duplicate_leaders",
    "normalise",
]
