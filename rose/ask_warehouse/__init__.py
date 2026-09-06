"""Natural-language → Redshift SQL pilot, grounded on a curated Northwind Retail schema.

Mounted only on the ``welcome`` module for the pilot. See
``common_py/ask_warehouse/mod.py`` for path constants and
``router.py`` for the FastAPI surface.
"""

from common_py.ask_warehouse.router import ask_router
from common_py.ask_warehouse.service import AskService

__all__ = ["ask_router", "AskService"]
