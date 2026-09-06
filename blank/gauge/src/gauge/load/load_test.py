"""Unit tests for the load driver: the rps derivation and drive_load's
round-robin scheduling. The network hit (`_hit`) is monkeypatched, so no server
is needed and the request counts stay deterministic."""

from __future__ import annotations

import threading
from collections import Counter

import pytest

# `_hit` is monkeypatched where it is defined — the inner module, not the package
# that re-exports drive_load — so drive_load's global lookup sees the stand-in.
from gauge.load import LoadResult, drive_load
from gauge.load import load as load_mod

# --- LoadResult.rps ---------------------------------------------------------


def test_rps_divides_ok_by_seconds():
    assert LoadResult(ok=100, total=100, seconds=2.0).rps == 50.0


def test_rps_is_zero_when_no_time_elapsed():
    # Guards against ZeroDivisionError when a run registers no elapsed time.
    assert LoadResult(ok=5, total=5, seconds=0.0).rps == 0.0


# --- drive_load -------------------------------------------------------------


def test_drive_load_rejects_empty_url_list():
    with pytest.raises(ValueError):
        drive_load([], requests=10, concurrency=2)


def test_drive_load_wraps_single_url(monkeypatch):
    seen: list[str] = []
    monkeypatch.setattr(load_mod, "_hit", lambda url: seen.append(url) or True)
    res = drive_load("http://h/", requests=3, concurrency=1)
    assert res.n_endpoints == 1
    assert seen == ["http://h/"] * 3


def test_drive_load_cycles_urls_round_robin(monkeypatch):
    lock = threading.Lock()
    seen: list[str] = []

    def fake_hit(url: str) -> bool:
        with lock:
            seen.append(url)
        return True

    monkeypatch.setattr(load_mod, "_hit", fake_hit)
    res = drive_load(["a", "b", "c"], requests=5, concurrency=4)

    # 5 requests over 3 urls: a,b,c,a,b -> a:2 b:2 c:1, regardless of thread order.
    assert Counter(seen) == Counter({"a": 2, "b": 2, "c": 1})
    assert res.ok == 5 and res.total == 5 and res.n_endpoints == 3


def test_drive_load_counts_only_successful_hits(monkeypatch):
    # Every other request fails -> ok reflects successes, total reflects attempts.
    monkeypatch.setattr(load_mod, "_hit", lambda url: url.endswith("ok"))
    res = drive_load(["x-ok", "x-bad"], requests=4, concurrency=2)
    assert res.total == 4
    assert res.ok == 2  # the two "x-ok" slots
