"""Troy: runtime consumers of the contract registry.

The registry (../registry) is the single source of truth. This package never
defines schemas — it loads them and enforces them at the two JSON boundaries:
ingestion (untrusted input) and dbt marts (data at rest).
"""

from pathlib import Path

# registry/ lives two levels up from this file: python/troy/__init__.py
REGISTRY = Path(__file__).resolve().parents[2] / "registry"
SAMPLES = Path(__file__).resolve().parents[1] / "samples"
