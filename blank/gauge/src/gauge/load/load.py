"""A minimal concurrent HTTP load driver (stdlib only).

Exposed as a function so `gauge run` can drive traffic in-process, and as the
`gauge load` subcommand for standalone use. Accepts one URL or many — when given
several (e.g. derived from an OpenAPI spec) it hits them round-robin.
"""

from __future__ import annotations

import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass


@dataclass
class LoadResult:
    ok: int
    total: int
    seconds: float
    n_endpoints: int = 1

    @property
    def rps(self) -> float:
        return self.ok / self.seconds if self.seconds else 0.0


def _hit(url: str) -> bool:
    try:
        with urllib.request.urlopen(url, timeout=10) as r:
            r.read()
        return True
    except Exception:
        return False


def drive_load(urls: str | list[str], requests: int, concurrency: int) -> LoadResult:
    """Fire `requests` GETs across `concurrency` threads, cycling through `urls`."""
    if isinstance(urls, str):
        urls = [urls]
    if not urls:
        raise ValueError("no URLs to load")
    schedule = [urls[i % len(urls)] for i in range(requests)]

    t0 = time.monotonic()
    with ThreadPoolExecutor(max_workers=concurrency) as pool:
        results = list(pool.map(_hit, schedule))
    return LoadResult(
        ok=sum(results),
        total=requests,
        seconds=time.monotonic() - t0,
        n_endpoints=len(urls),
    )
