"""The VLM wrapper — see :mod:`extract.extractor.extractor`."""

from extract.extractor.extractor import (
    DEFAULT_MODEL,
    MODELS,
    ModelChoice,
    VisionLanguageExtractor,
    available_memory_gb,
    recommend_model,
    resolve,
    resolve_model,
)

__all__ = [
    "DEFAULT_MODEL",
    "MODELS",
    "ModelChoice",
    "VisionLanguageExtractor",
    "available_memory_gb",
    "recommend_model",
    "resolve",
    "resolve_model",
]
