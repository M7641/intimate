"""Bootstrap the tools gauge needs — primarily a working container runtime.

`gauge setup` calls `ensure_runtime`, which is idempotent: it first checks
whether docker or podman already answers, and only installs / initializes when
nothing works. On macOS it installs podman via Homebrew and brings up a podman
machine; on native Linux podman needs no VM. Anything that changes the system
(a `brew install`, a `machine init`) asks first unless `--yes` is passed.
"""

from __future__ import annotations

import platform
import shutil
import subprocess
from collections.abc import Callable

from gauge.runner import detect_runtime

Echo = Callable[[str], None]
Confirm = Callable[[str], bool]


def _run(cmd: list[str], echo: Echo) -> int:
    echo(f"$ {' '.join(cmd)}")
    return subprocess.run(cmd).returncode


def ensure_runtime(prefer: str, assume_yes: bool, echo: Echo, confirm: Confirm) -> str:
    """Make a container runtime available, installing podman if needed.
    Returns the working runtime name, or raises RuntimeError with guidance."""
    # 1. Already working? Then there is nothing to do.
    try:
        rt = detect_runtime(None)
        echo(f"{rt} is already working — nothing to do.")
        return rt
    except RuntimeError:
        pass

    if prefer == "docker":
        raise RuntimeError(
            "Docker is installed but not running. Start Docker Desktop and re-run, "
            "or `gauge setup --runtime podman` to use podman instead."
        )

    system = platform.system()

    # 2. Install the podman binary if it's missing.
    if not shutil.which("podman"):
        if system == "Darwin":
            if not shutil.which("brew"):
                raise RuntimeError(
                    "Homebrew not found — install it from https://brew.sh, or install podman manually."
                )
            if not (
                assume_yes or confirm("Install podman with `brew install podman`?")
            ):
                raise RuntimeError("aborted.")
            if _run(["brew", "install", "podman"], echo) != 0:
                raise RuntimeError("`brew install podman` failed.")
        else:
            raise RuntimeError(
                f"podman is not installed. Install it with your package manager on {system} "
                "(e.g. `sudo apt install podman` or `sudo dnf install podman`), then re-run."
            )

    # 3. macOS/Windows run containers in a VM ("machine"); Linux does not.
    if system in ("Darwin", "Windows"):
        _ensure_machine(assume_yes, echo, confirm)

    # 4. Verify it now works.
    rt = detect_runtime("podman")
    echo("podman is ready.")
    return rt


def _ensure_machine(assume_yes: bool, echo: Echo, confirm: Confirm) -> None:
    lst = subprocess.run(
        ["podman", "machine", "list", "--format", "{{.Running}}"],
        capture_output=True,
        text=True,
    )
    states = [s.strip().lower() for s in lst.stdout.splitlines() if s.strip()]

    if not states:
        if not (
            assume_yes
            or confirm(
                "No podman machine exists. Create one now? (downloads a VM image)"
            )
        ):
            raise RuntimeError("aborted.")
        if _run(["podman", "machine", "init"], echo) != 0:
            raise RuntimeError("`podman machine init` failed.")
        states = ["false"]

    if "true" not in states:
        echo("starting the podman machine ...")
        if _run(["podman", "machine", "start"], echo) != 0:
            raise RuntimeError("`podman machine start` failed.")
