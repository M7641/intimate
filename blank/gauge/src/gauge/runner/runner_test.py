"""Unit + property tests for the measurement core, exercised without a real
container: `_exec` (the single `<rt> exec` round-trip) is monkeypatched to feed
crafted cgroup output, so we test the v2/v1 parsing and the ContainerGone path
directly. Run.summary is pure arithmetic and tested as-is."""

from __future__ import annotations

import math

import pytest
from hypothesis import given
from hypothesis import strategies as st

# `_exec` is monkeypatched on the inner module where read_cgroup looks it up.
from gauge.runner import ContainerGone, Run, Sample, read_cgroup
from gauge.runner import runner as runner_mod

# --- read_cgroup: cgroup v2 -> v1 fallback -> ContainerGone -----------------


def _fake_exec(v2: str, v1: str = ""):
    """Return an `_exec` stand-in that answers by cgroup version. v1/v2 are
    distinguished by the `cpuacct` path only present in the v1 script."""

    def _exec(rt, cid, script):
        return v1 if "cpuacct" in script else v2

    return _exec


def test_read_cgroup_parses_v2(monkeypatch):
    v2 = "usage_usec 123456\nuser_usec 100000\n@@M\n2097152\n@@P\n3145728\n"
    monkeypatch.setattr(runner_mod, "_exec", _fake_exec(v2))
    read = read_cgroup("podman", "cid")
    assert read.cpu_usec == 123456
    assert read.mem_current == 2097152
    assert read.mem_peak == 3145728


def test_read_cgroup_falls_back_to_v1_when_v2_empty(monkeypatch):
    # v2 present but its memory section is blank -> retry against v1 paths.
    v2 = "\n@@M\n\n@@P\n\n"
    v1 = "5000000\n@@M\n1048576\n@@P\n2097152\n"  # cpuacct.usage is nanoseconds
    monkeypatch.setattr(runner_mod, "_exec", _fake_exec(v2, v1))
    read = read_cgroup("docker", "cid")
    assert read.cpu_usec == 5000  # 5_000_000 ns // 1000 -> usec
    assert read.mem_current == 1048576
    assert read.mem_peak == 2097152


def test_read_cgroup_raises_when_container_gone(monkeypatch):
    # No markers at all -> `exec` itself failed, the container is gone.
    monkeypatch.setattr(runner_mod, "_exec", _fake_exec("", ""))
    with pytest.raises(ContainerGone):
        read_cgroup("podman", "cid")


# --- Run.summary: exact figures from the sample timeline --------------------


def _run_with(samples, *, peak_bytes, total_cpu_usec, duration_s) -> Run:
    r = Run(image="img", runtime="podman")
    r.samples = samples
    r.peak_bytes = peak_bytes
    r.total_cpu_usec = total_cpu_usec
    r.duration_s = duration_s
    return r


def test_summary_computes_trapezoidal_mb_seconds_and_cpu():
    samples = [
        Sample(t=0.0, mem_mb=100.0, cpu_cores=0.5),
        Sample(t=1.0, mem_mb=200.0, cpu_cores=1.0),
        Sample(t=2.0, mem_mb=200.0, cpu_cores=0.0),
    ]
    r = _run_with(
        samples,
        peak_bytes=200 * 1024 * 1024,
        total_cpu_usec=2_000_000,  # 2 CPU-seconds
        duration_s=2.0,
    )
    s = r.summary()
    assert s["exact_peak_mem_mb"] == 200.0
    assert s["sampled_mean_mem_mb"] == 166.7  # (100+200+200)/3
    # trapezoids: (100+200)/2*1 + (200+200)/2*1 = 150 + 200
    assert s["mb_seconds"] == 350.0
    assert s["exact_cpu_seconds"] == 2.0
    assert s["peak_cpu_cores"] == 1.0
    assert s["mean_cpu_cores"] == 1.0  # 2 CPU-s over 2 s wall
    assert s["n_samples"] == 3


def test_summary_handles_empty_run_without_dividing_by_zero():
    s = Run(image="img", runtime="podman").summary()
    assert s["exact_peak_mem_mb"] == 0.0
    assert s["sampled_mean_mem_mb"] == 0.0
    assert s["mb_seconds"] == 0.0
    assert s["mean_cpu_cores"] == 0.0
    assert s["n_samples"] == 0


@given(
    st.lists(
        st.tuples(
            st.floats(min_value=0, max_value=1e4),  # mem_mb
            st.floats(min_value=0, max_value=64),  # cpu_cores
        ),
        min_size=1,
        max_size=30,
    )
)
def test_summary_mb_seconds_bounded_by_extremes(rows):
    # With a fixed 1s cadence, the memory-time integral must sit between the
    # trough and the crest scaled by the elapsed span — a trapezoid can't leave
    # the box its endpoints define.
    samples = [
        Sample(t=float(i), mem_mb=m, cpu_cores=c) for i, (m, c) in enumerate(rows)
    ]
    r = _run_with(
        samples, peak_bytes=0, total_cpu_usec=0, duration_s=float(len(rows) - 1)
    )
    mb_seconds = r.summary()["mb_seconds"]
    span = len(rows) - 1
    lo = min(m for m, _ in rows) * span
    hi = max(m for m, _ in rows) * span
    # rounding to 1 decimal can nudge the bound by up to 0.05.
    assert lo - 0.1 <= mb_seconds <= hi + 0.1
    assert not math.isnan(mb_seconds)
