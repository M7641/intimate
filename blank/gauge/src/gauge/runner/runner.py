"""Core measurement logic: run an app in a container and read the kernel's own
cgroup accounting.

Where an external sampler only *peeks* at RSS periodically, this reads the
counters the kernel maintains continuously — `memory.peak` (an exact
high-watermark) and `cpu.stat`'s `usage_usec` (exact cumulative CPU
microseconds). Those are what the OOM-killer and the scheduler actually use, so
the numbers are the real ones, isolated from everything else on the host.

Runtime-agnostic: works with `docker` or `podman` (compatible CLIs, same cgroup
files). Reads cgroup v2 (Docker Desktop, Podman machine, modern Linux); falls
back to v1.

This module has no CLI and no user-facing formatting — it takes an
`on_event` callback for progress and returns a `Run`. The Typer layer in
`cli.py` owns all presentation.
"""

from __future__ import annotations

import shutil
import subprocess
import time
import urllib.error
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass, field


@dataclass
class Sample:
    t: float
    mem_mb: float  # memory.current — instantaneous usage
    cpu_cores: float  # CPU utilization over the last interval (exact, from usage_usec)


@dataclass
class CgroupRead:
    """One raw read of the kernel counters inside the container."""

    mem_current: int  # bytes
    mem_peak: int  # bytes — monotonic high-watermark
    cpu_usec: int  # cumulative CPU microseconds


class ContainerGone(Exception):
    """Raised when the container can no longer be read — it exited or was
    OOM-killed. The sampling loop catches this and stops cleanly."""


# One shell command reads every counter in a single `exec` round-trip.
# `2>/dev/null` on each so a missing v2 file just yields an empty section,
# which we detect and retry against v1 paths.
_READ_V2 = (
    "cat /sys/fs/cgroup/cpu.stat 2>/dev/null; echo @@M; "
    "cat /sys/fs/cgroup/memory.current 2>/dev/null; echo @@P; "
    "cat /sys/fs/cgroup/memory.peak 2>/dev/null"
)
_READ_V1 = (
    "cat /sys/fs/cgroup/cpuacct/cpuacct.usage 2>/dev/null; echo @@M; "
    "cat /sys/fs/cgroup/memory/memory.usage_in_bytes 2>/dev/null; echo @@P; "
    "cat /sys/fs/cgroup/memory/memory.max_usage_in_bytes 2>/dev/null"
)


def detect_runtime(preferred: str | None = None) -> str:
    """Pick a working container runtime. Honour an explicit choice, else try
    whichever of docker/podman actually answers `info` (has a live daemon)."""
    candidates = [preferred] if preferred else ["docker", "podman"]
    for rt in candidates:
        if rt and shutil.which(rt):
            if subprocess.run([rt, "info"], capture_output=True).returncode == 0:
                return rt
    raise RuntimeError(
        f"no working container runtime ({preferred or 'docker/podman'}). "
        "Start Docker Desktop, or run `podman machine start`."
    )


def _exec(rt: str, cid: str, script: str) -> str:
    out = subprocess.run(
        [rt, "exec", cid, "sh", "-c", script], capture_output=True, text=True
    )
    return out.stdout


def _tail_logs(rt: str, name: str, lines: int = 20) -> str:
    """Last few lines of the container's logs — shown when it dies unexpectedly."""
    out = subprocess.run(
        [rt, "logs", "--tail", str(lines), name], capture_output=True, text=True
    )
    return (out.stdout + out.stderr).strip()


def read_cgroup(rt: str, cid: str) -> CgroupRead:
    """Read the exact counters from inside the container, cgroup v2 then v1."""
    raw = _exec(rt, cid, _READ_V2)
    version = 2
    if "@@M" not in raw or raw.split("@@M")[1].split("@@P")[0].strip() == "":
        raw = _exec(rt, cid, _READ_V1)
        version = 1
    # No markers at all means `exec` itself failed — the container is gone.
    if "@@M" not in raw or "@@P" not in raw:
        raise ContainerGone
    cpu_part, rest = raw.split("@@M", 1)
    mem_cur_part, mem_peak_part = rest.split("@@P", 1)

    if version == 2:
        cpu_usec = 0
        for line in cpu_part.splitlines():
            if line.startswith("usage_usec"):
                cpu_usec = int(line.split()[1])
        mem_current = int(mem_cur_part.strip() or 0)
        mem_peak = int(mem_peak_part.strip() or 0)
    else:
        # v1: cpuacct.usage is nanoseconds; convert to usec for a common unit.
        cpu_usec = int(cpu_part.strip() or "0") // 1000
        mem_current = int(mem_cur_part.strip() or 0)
        mem_peak = int(mem_peak_part.strip() or 0)

    return CgroupRead(mem_current=mem_current, mem_peak=mem_peak, cpu_usec=cpu_usec)


@dataclass
class Run:
    image: str
    runtime: str
    samples: list[Sample] = field(default_factory=list)
    peak_bytes: int = 0
    total_cpu_usec: int = 0
    duration_s: float = 0.0
    stopped_reason: str = "load finished"

    def summary(self) -> dict:
        mem = [s.mem_mb for s in self.samples] or [0]
        cores = [s.cpu_cores for s in self.samples] or [0]
        mb_seconds = 0.0
        for a, b in zip(self.samples, self.samples[1:]):
            mb_seconds += (a.mem_mb + b.mem_mb) / 2 * (b.t - a.t)
        cpu_seconds = self.total_cpu_usec / 1_000_000
        return {
            "image": self.image,
            "runtime": self.runtime,
            "duration_s": round(self.duration_s, 2),
            "exact_peak_mem_mb": round(self.peak_bytes / 1024 / 1024, 1),
            "sampled_mean_mem_mb": round(sum(mem) / len(mem), 1),
            "mb_seconds": round(mb_seconds, 1),
            "exact_cpu_seconds": round(cpu_seconds, 3),
            "peak_cpu_cores": round(max(cores), 2),
            "mean_cpu_cores": round(cpu_seconds / self.duration_s, 2)
            if self.duration_s
            else 0.0,
            "n_samples": len(self.samples),
            "stopped": self.stopped_reason,
        }


def wait_until_ready(url: str, timeout: float) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=1):
                return True
        except (urllib.error.URLError, ConnectionError, OSError):
            time.sleep(0.2)
    return False


def _noop(_: str) -> None:
    pass


def run(
    rt: str,
    image: str,
    port: int,
    host_port: int,
    interval: float = 0.5,
    ready_url: str | None = None,
    ready_timeout: float = 30.0,
    load: Callable[[], None] | None = None,
    max_duration: float | None = None,
    run_args: list[str] | None = None,
    on_event: Callable[[str], None] = _noop,
) -> Run:
    """Launch `image`, sample its cgroup counters until the load finishes (or the
    container stops / times out), and return the exact figures.

    `load` is a zero-arg callable started once the app is ready; it runs in a
    background thread so sampling continues while it drives traffic.
    """
    import threading

    run_args = run_args or []
    name = f"gauge_run_{port}_{host_port}"
    subprocess.run([rt, "rm", "-f", name], capture_output=True)  # clear any stale one
    cid = subprocess.run(
        [
            rt,
            "run",
            "-d",
            "--name",
            name,
            "-p",
            f"{host_port}:{port}",
            *run_args,
            image,
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    on_event(f"{rt}: started container {cid[:12]} from {image}")

    result = Run(image=image, runtime=rt)
    load_thread: threading.Thread | None = None
    started_load = False
    t0 = time.monotonic()

    # The container may not accept `exec` the instant it starts — retry briefly.
    # But if it has already exited (crashed on startup), stop waiting and say why.
    base = None
    for _ in range(20):
        try:
            base = read_cgroup(rt, cid)
            break
        except ContainerGone:
            running = subprocess.run(
                [rt, "inspect", "-f", "{{.State.Running}}", cid],
                capture_output=True,
                text=True,
            ).stdout.strip()
            if running != "true":
                break
            time.sleep(0.2)
    if base is None:
        detail = _tail_logs(rt, name)
        subprocess.run([rt, "rm", "-f", name], capture_output=True)
        msg = "container crashed on startup before it could be measured."
        if detail:
            msg += f"\n--- last container logs ---\n{detail}"
        raise RuntimeError(msg)
    prev_usec = base.cpu_usec

    try:
        while True:
            t = time.monotonic() - t0

            if load and not started_load:
                if ready_url is None or wait_until_ready(ready_url, ready_timeout):
                    on_event("app ready — starting load")
                    load_thread = threading.Thread(target=load, daemon=True)
                    load_thread.start()
                started_load = True

            try:
                cur = read_cgroup(rt, cid)
            except ContainerGone:
                oom = subprocess.run(
                    [rt, "inspect", "-f", "{{.State.OOMKilled}}", cid],
                    capture_output=True,
                    text=True,
                ).stdout.strip()
                result.stopped_reason = "OOM-killed" if oom == "true" else "exited"
                on_event(f"container {result.stopped_reason} during the run")
                break

            cores = 0.0
            if result.samples:
                dt = t - result.samples[-1].t
                cores = (cur.cpu_usec - prev_usec) / 1_000_000 / dt if dt > 0 else 0.0
            prev_usec = cur.cpu_usec

            result.samples.append(
                Sample(
                    t=round(t, 3),
                    mem_mb=round(cur.mem_current / 1024 / 1024, 2),
                    cpu_cores=round(cores, 2),
                )
            )
            result.peak_bytes = cur.mem_peak
            result.total_cpu_usec = cur.cpu_usec - base.cpu_usec
            result.duration_s = t

            if (
                load
                and started_load
                and load_thread is not None
                and not load_thread.is_alive()
            ):
                result.stopped_reason = "load finished"
                on_event("load finished")
                break
            if max_duration is not None and t >= max_duration:
                result.stopped_reason = "max duration"
                on_event("max duration reached")
                break
            state = subprocess.run(
                [rt, "inspect", "-f", "{{.State.Running}}", cid],
                capture_output=True,
                text=True,
            ).stdout.strip()
            if state != "true":
                result.stopped_reason = "container stopped"
                on_event("container stopped")
                break

            time.sleep(interval)
    finally:
        subprocess.run([rt, "rm", "-f", name], capture_output=True)
    return result
