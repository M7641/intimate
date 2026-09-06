"""Minimal concurrent HTTP load driver (stdlib only)."""

from .load import LoadResult, drive_load

__all__ = ["LoadResult", "drive_load"]
