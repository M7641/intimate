"""Show WHAT is in the FastAPI app's RAM — not just how much.

gauge answers "how much memory" from the outside via cgroups. This answers "what
is that memory" from the inside via Python's `tracemalloc`: it runs the app with
allocation tracking on, drives some `/work` load to accumulate objects, then asks
the app which source lines hold the resident bytes. This is Python-specific, so
it lives in the example rather than in gauge core.

Run it:  uv run python examples/fastapi/introspect.py

It reuses gauge's own runtime detection and load driver, but leaves the container
up long enough to query the in-app `/debug/memory` endpoint.
"""

from __future__ import annotations

import json
import subprocess
import urllib.request
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

from gauge.load import drive_load  # noqa: E402
from gauge.runner import detect_runtime, wait_until_ready  # noqa: E402

IMAGE = "gauge-fastapi-example"
HERE = Path(__file__).parent
PORT = 8081  # a distinct host port, so it won't clash with a `gauge run`
WARMUP_REQUESTS = 200  # each /work retains 256 KB → ~50 MB of clearly-attributable RAM


def main() -> None:
    rt = detect_runtime(None)
    print(f"building {IMAGE} ...")
    subprocess.run(
        [rt, "build", "-t", IMAGE, "-f", str(HERE / "Dockerfile"), str(HERE)],
        check=True,
    )

    name = "gauge_fastapi_introspect"
    subprocess.run([rt, "rm", "-f", name], capture_output=True)
    # Single worker + tracemalloc: one process holds all the objects, so the
    # snapshot accounts for everything the app allocated.
    subprocess.run(
        [
            rt,
            "run",
            "-d",
            "--name",
            name,
            "-p",
            f"{PORT}:8080",
            "-e",
            "GAUGE_HOST=0.0.0.0",
            "-e",
            "WORKERS=1",
            "-e",
            "GAUGE_TRACEMALLOC=1",
            IMAGE,
        ],
        check=True,
    )
    try:
        base = f"http://127.0.0.1:{PORT}"
        if not wait_until_ready(base + "/health", 30):
            raise SystemExit("app did not become ready")

        res = drive_load(base + "/work", WARMUP_REQUESTS, 8)
        print(f"warmed with {res.ok}/{res.total} /work requests")

        with urllib.request.urlopen(base + "/debug/memory?limit=12") as r:
            data = json.load(r)
        _report(data)
    finally:
        subprocess.run([rt, "rm", "-f", name], capture_output=True)


def _report(data: dict) -> None:
    if "error" in data:
        raise SystemExit(data["error"])

    print(
        f"\nRSS {data['rss_mb']} MB  ·  tracemalloc accounts for {data['traced_mb']} MB "
        f"(peak {data['peak_traced_mb']} MB)\n"
    )
    print(f"{'size MB':>9}  {'count':>8}  where")
    for row in data["top"]:
        print(f"{row['size_mb']:>9}  {row['count']:>8}  {row['file']}:{row['line']}")

    top = data["top"][:10][::-1]  # reverse so the largest bar is on top
    labels = [f"{r['file']}:{r['line']}" for r in top]
    sizes = [r["size_mb"] for r in top]

    fig, ax = plt.subplots(figsize=(9, 4.8))
    ax.barh(labels, sizes, color="#4c72b0")
    ax.set_xlabel("MB tracked in RAM")
    ax.set_title(
        f"What's in RAM — top allocation sites  (RSS {data['rss_mb']} MB, "
        f"{data['traced_mb']} MB traced)",
        fontsize=10,
    )
    ax.margins(y=0.01)
    fig.tight_layout()
    out = HERE / "report" / "whats_in_ram.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out, dpi=120)
    print(f"\nchart written to {out}")


if __name__ == "__main__":
    main()
