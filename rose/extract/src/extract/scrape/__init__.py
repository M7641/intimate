"""Product-page scraper (test-data builder) — see :mod:`extract.scrape.scrape`.

Deliberately not re-exported from the top-level package: the extraction engine
never depends on it.
"""

from extract.scrape.scrape import main

__all__ = ["main"]
