"""Domains are pure data; the registry is the only behaviour worth pinning."""

from __future__ import annotations

import pytest

from extract.domains import SCHEMAS, get_schema


def test_known_domains_are_registered():
    assert {"clothing", "electronics"} <= set(SCHEMAS)


def test_get_schema_returns_the_registered_schema():
    assert get_schema("clothing").domain.startswith("clothing")


def test_get_schema_unknown_raises_with_available_list():
    with pytest.raises(KeyError, match="Unknown domain"):
        get_schema("furniture")


def test_conditional_children_surface_as_columns():
    # A motherboard unlocks chipset/socket; those must be enumerable columns.
    cols = get_schema("electronics").column_names()
    assert {"chipset", "socket"} <= set(cols)
