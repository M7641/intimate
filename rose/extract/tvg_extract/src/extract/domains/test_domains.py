"""Domains are pure data; the registry is the only behaviour worth pinning."""

from __future__ import annotations

import pytest

from extract.domains import SCHEMAS, get_schema


def test_known_domains_are_registered():
    assert {"clothing", "electronics", "furniture"} <= set(SCHEMAS)


def test_get_schema_returns_the_registered_schema():
    assert get_schema("clothing").domain.startswith("clothing")


def test_get_schema_unknown_raises_with_available_list():
    with pytest.raises(KeyError, match="Unknown domain"):
        get_schema("nonexistent")


def test_conditional_children_surface_as_columns():
    # A motherboard unlocks chipset/socket; those must be enumerable columns.
    cols = get_schema("electronics").column_names()
    assert {"chipset", "socket"} <= set(cols)


def test_furniture_conditional_children_surface_as_columns():
    # A bed unlocks bed_size/bed_type; a sofa unlocks sofa_type/seat_count.
    cols = get_schema("furniture").column_names()
    assert {"bed_size", "bed_type", "sofa_type", "seat_count"} <= set(cols)
