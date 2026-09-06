"""Unit + property tests for the sparkline renderer. The rest of cli.py is Typer
wiring and rich formatting over runner.run — exercised end-to-end, not unit
tested — so only the pure `_sparkline` helper is covered here."""

from __future__ import annotations

from hypothesis import given
from hypothesis import strategies as st

from gauge.cli.cli import _BLOCKS, _sparkline  # private helpers from the module


def test_sparkline_empty_is_blank():
    assert _sparkline([]) == ""


def test_sparkline_maps_max_to_full_block():
    assert _sparkline([1.0])[-1] == "█"
    assert _sparkline([1, 2, 4])[-1] == "█"  # the crest is always the fullest block


def test_sparkline_all_zero_does_not_divide_by_zero():
    # max([0,0]) is 0 -> the `or 1.0` guard keeps every bar at the low block.
    assert _sparkline([0, 0]) == "▁▁"


@given(st.lists(st.floats(min_value=0, max_value=1e6), min_size=1, max_size=64))
def test_sparkline_length_and_alphabet(values):
    out = _sparkline(values)
    assert len(out) == len(values)
    assert all(ch in _BLOCKS for ch in out)
