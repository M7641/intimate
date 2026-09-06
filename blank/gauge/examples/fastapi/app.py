"""A small FastAPI app — a realistic thing to point gauge at.

FastAPI auto-generates an OpenAPI spec at /openapi.json, which lets gauge derive
its load from the documented endpoints instead of a single hard-coded URL. The
endpoints below deliberately span shapes gauge's OpenAPI reader handles: a plain
GET, a path parameter, and a required query parameter. `/work` does real CPU +
memory work so the load has a measurable footprint.
"""

from __future__ import annotations

import hashlib
import os
import tracemalloc

# Start allocation tracking BEFORE importing FastAPI, so even import-time
# allocations (pydantic models, route tables) are attributed to their source.
# Opt-in because it adds overhead — the introspect.py helper sets this env.
if os.environ.get("GAUGE_TRACEMALLOC"):
    tracemalloc.start(int(os.environ.get("GAUGE_TRACEMALLOC_FRAMES", "20")))

import uvicorn  # noqa: E402
from fastapi import FastAPI  # noqa: E402

app = FastAPI(title="gauge FastAPI example")

_cache: list[bytearray] = []  # grows on every /work request → visible memory climb


@app.get("/health")
def health() -> dict:
    return {"status": "ok"}


@app.get("/items/{item_id}")
def get_item(item_id: int) -> dict:
    return {"item_id": item_id, "name": f"item-{item_id}"}


@app.get("/search")
def search(q: str) -> dict:
    return {"query": q, "results": [f"{q}-{i}" for i in range(5)]}


@app.get("/work")
def work() -> dict:
    """CPU + memory work, so the load driver produces a real footprint."""
    block = b"x" * 64_000
    digest = block
    for _ in range(300):
        digest = hashlib.sha256(digest + block).digest()
    _cache.append(bytearray(256_000))
    return {"cache": len(_cache), "last": digest[:8].hex()}


def _rss_mb() -> float:
    """This process's resident memory, from the kernel — the total to explain."""
    try:
        with open("/proc/self/status") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    return round(int(line.split()[1]) / 1024, 1)  # kB → MB
    except OSError:
        pass
    return 0.0


def _short(path: str) -> str:
    """Trim noisy absolute prefixes so the location reads as a module path."""
    for marker in ("site-packages/", "/app/"):
        if marker in path:
            return path.split(marker, 1)[1]
    return path


@app.get("/debug/memory", include_in_schema=False)
def debug_memory(limit: int = 12) -> dict:
    """What is actually in this worker's RAM, by allocation site (tracemalloc).

    Excluded from the OpenAPI schema so gauge's OpenAPI load ignores it. Returns
    the process RSS, how much of it tracemalloc accounts for, and the top source
    lines holding memory."""
    if not tracemalloc.is_tracing():
        return {
            "error": "tracemalloc not enabled — start the app with GAUGE_TRACEMALLOC=1"
        }
    snapshot = tracemalloc.take_snapshot()
    traced, peak = tracemalloc.get_traced_memory()
    top = [
        {
            "file": _short(stat.traceback[0].filename),
            "line": stat.traceback[0].lineno,
            "size_mb": round(stat.size / 1024 / 1024, 3),
            "count": stat.count,
        }
        for stat in snapshot.statistics("lineno")[:limit]
    ]
    return {
        "rss_mb": _rss_mb(),
        "traced_mb": round(traced / 1024 / 1024, 2),
        "peak_traced_mb": round(peak / 1024 / 1024, 2),
        "top": top,
    }


def main() -> None:
    port = int(os.environ.get("PORT", "8080"))
    host = os.environ.get("GAUGE_HOST", "127.0.0.1")
    workers = int(os.environ.get("WORKERS", "8"))
    # Pass the app as an IMPORT STRING, not the object: uvicorn re-imports it in
    # each forked worker, so `workers > 1` only works this way. "app:app" resolves
    # because this file's directory is on sys.path when run as `python app.py`.
    uvicorn.run("app:app", host=host, port=port, log_level="warning", workers=workers)


if __name__ == "__main__":
    main()
