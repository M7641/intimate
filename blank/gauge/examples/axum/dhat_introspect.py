"""Profile WHAT is in the Axum app's RAM with dhat — the Rust analogue of memray.

Rust has no tracemalloc/memray, but `dhat` (dhat-rs) fills the same role: a global
allocator records every allocation and writes a `dhat-heap.json` capture, which
you open in the DHAT viewer to see allocation sites by bytes. It runs the app
built `--features dhat-heap`, drives load, stops it gracefully so dhat flushes,
then surfaces the capture and dhat's own summary line.

Run it:  uv run python examples/axum/dhat_introspect.py
Then open examples/axum/report/dhat/dhat-heap.json at
https://nnethercote.github.io/dh_view/dh_view.html

Language-specific, so it lives in the example rather than in gauge core.
"""

from __future__ import annotations

import subprocess
from pathlib import Path

from gauge.load import drive_load
from gauge.runner import detect_runtime, wait_until_ready

IMAGE = "gauge-axum-dhat"
HERE = Path(__file__).parent
OUT_DIR = (HERE / "report" / "dhat").resolve()
PORT = 8083
WARMUP_REQUESTS = 200


def main() -> None:
    rt = detect_runtime(None)
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    print(f"building {IMAGE} (this compiles Rust with dhat) ...")
    subprocess.run(
        [rt, "build", "-t", IMAGE, "-f", str(HERE / "Dockerfile.dhat"), str(HERE)],
        check=True,
    )

    name = "gauge_axum_dhat"
    subprocess.run([rt, "rm", "-f", name], capture_output=True)
    # Mount the host report dir at /captures (the app's cwd), so dhat-heap.json
    # lands on the host directly.
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

        # Graceful stop → main() returns → the dhat Profiler drops → capture flushes.
        print("stopping app so dhat finalizes the capture ...")
        subprocess.run([rt, "stop", "--time", "25", name], check=True)

        # dhat prints its summary to stderr on drop — surface it from the logs.
        logs = subprocess.run([rt, "logs", name], capture_output=True, text=True)
        summary = "\n".join(
            line
            for line in (logs.stdout + logs.stderr).splitlines()
            if line.startswith("dhat:")
        )
        print("\n" + (summary or "(no dhat summary found in logs)"))
    finally:
        subprocess.run([rt, "rm", "-f", name], capture_output=True)

    print(f"\ncapture: {OUT_DIR / 'dhat-heap.json'}")
    print("open it at https://nnethercote.github.io/dh_view/dh_view.html")


if __name__ == "__main__":
    main()
