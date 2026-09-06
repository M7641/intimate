"""Bootstrap the tools gauge needs — primarily a working container runtime."""

from .setup import ensure_runtime

__all__ = ["ensure_runtime"]
