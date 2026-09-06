"""Profile WHAT is in the FastAPI app's RAM with memray — native allocations too.

Where tracemalloc (see introspect.py) only sees Python-level allocations, memray
also captures NATIVE ones — the C memory that pydantic-core, uvicorn, and the
allocator itself hold, which is most of the gap between "traced" and RSS. It runs
the app under `memray run`, drives load, stops it gracefully so memray finalizes
the capture, then renders an interactive flamegraph.

Run it:  uv run python examples/fastapi/memray_introspect.py
Then open examples/fastapi/report/memray/flamegraph.html

Python-specific, so it lives in the example rather than in gauge core.
"""

from __future__ import annotations

import subprocess
from pathlib import Path

from gauge.load import drive_load
from gauge.runner import detect_runtime, wait_until_ready

IMAGE = "gauge-fastapi-memray"
HERE = Path(__file__).parent
OUT_DIR = (HERE / "report" / "memray").resolve()
PORT = 8082
WARMUP_REQUESTS = 200


def main() -> None:
    rt = detect_runtime(None)
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    print(f"building {IMAGE} ...")
    subprocess.run(
        [rt, "build", "-t", IMAGE, "-f", str(HERE / "Dockerfile.memray"), str(HERE)],
        check=True,
    )

    name = "gauge_fastapi_memray"
    subprocess.run([rt, "rm", "-f", name], capture_output=True)
    # Mount the host report dir at /captures so the .bin and flamegraph land on
    # the host without a `cp` dance.
    subprocess.run(
        [
            rt,
            "run",
            "-d",
            "--name",
            name,
            "-p",
            f"{PORT}:8080",
            "-v",
            f"{OUT_DIR}:/captures",
            IMAGE,
        ],
        check=True,
    )
    try:
        base = f"http://127.0.0.1:{PORT}"
        if not wait_until_ready(base + "/health", 40):
            raise SystemExit("app did not become ready")

        res = drive_load(base + "/work", WARMUP_REQUESTS, 8)
        print(f"warmed with {res.ok}/{res.total} /work requests")

        # Graceful stop → uvicorn shuts down → memray writes the capture footer.
        print("stopping app so memray finalizes the capture ...")
        subprocess.run([rt, "stop", "--time", "25", name], check=True)

        _render(rt)
    finally:
        subprocess.run([rt, "rm", "-f", name], capture_output=True)


def _render(rt: str) -> None:
    """Generate the flamegraph + a text summary from the capture, in-container."""
    flamegraph = subprocess.run(
        [
            rt,
            "run",
            "--rm",
            "-v",
            f"{OUT_DIR}:/captures",
            IMAGE,
            "memray",
            "flamegraph",
            "--force",
            "-o",
            "/captures/flamegraph.html",
            "/captures/out.bin",
        ],
        capture_output=True,
        text=True,
    )
    if flamegraph.returncode != 0:
        raise SystemExit(f"memray flamegraph failed:\n{flamegraph.stderr}")

    summary = subprocess.run(
        [
            rt,
            "run",
            "--rm",
            "-v",
            f"{OUT_DIR}:/captures",
            IMAGE,
            "memray",
            "summary",
            "/captures/out.bin",
        ],
        capture_output=True,
        text=True,
    )
    print(summary.stdout or summary.stderr)
    print(f"\nflamegraph: {OUT_DIR / 'flamegraph.html'}")
    print(f"capture:    {OUT_DIR / 'out.bin'}")


if __name__ == "__main__":
    main()
