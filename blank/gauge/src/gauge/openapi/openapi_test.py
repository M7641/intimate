"""Unit + property tests for OpenAPI -> concrete GET URL derivation.

No network here: `fetch_spec` (the one IO boundary) is left alone; we test the
pure `derive_get_urls` / `_example_for` against hand-built spec dicts.
"""

from __future__ import annotations

from hypothesis import given
from hypothesis import strategies as st

from gauge.openapi import derive_get_urls  # public API via package re-export
from gauge.openapi.openapi import _example_for  # private helper from the module

# --- _example_for: sample value by decreasing specificity --------------------


def test_example_for_prefers_param_level_example():
    assert _example_for({"example": 42, "schema": {"default": "x"}}) == "42"


def test_example_for_falls_back_to_schema_example_then_default():
    assert _example_for({"schema": {"example": "e"}}) == "e"
    assert _example_for({"schema": {"default": "d"}}) == "d"


def test_example_for_uses_first_enum_value():
    assert _example_for({"schema": {"enum": ["a", "b"]}}) == "a"


def test_example_for_type_based_defaults():
    assert _example_for({"schema": {"type": "integer"}}) == "1"
    assert _example_for({"schema": {"type": "boolean"}}) == "true"
    assert _example_for({}) == "example"  # no schema at all -> string default


# --- derive_get_urls --------------------------------------------------------

_SPEC = {
    "paths": {
        "/health": {"get": {}},
        "/items/{id}": {
            "get": {
                "parameters": [
                    {"name": "id", "in": "path", "schema": {"type": "integer"}}
                ]
            }
        },
        "/search": {
            "get": {
                "parameters": [
                    {
                        "name": "q",
                        "in": "query",
                        "required": True,
                        "schema": {"type": "string"},
                    },
                    {
                        "name": "page",
                        "in": "query",
                        "required": False,
                        "schema": {"type": "integer"},
                    },
                ]
            }
        },
        "/create": {"post": {}},  # not a GET -> skipped
        "/unfilled/{missing}": {"get": {}},  # path param we cannot sample -> skipped
    }
}


def test_derive_get_urls_covers_gets_and_fills_params():
    urls = derive_get_urls(_SPEC, "http://h/")
    assert set(urls) == {
        "http://h/health",
        "http://h/items/1",  # integer path param -> "1"
        "http://h/search?q=example",  # required query kept, optional dropped
    }


def test_derive_get_urls_strips_trailing_slash_on_base():
    assert derive_get_urls({"paths": {"/x": {"get": {}}}}, "http://h///") == [
        "http://h/x"
    ]


def test_derive_get_urls_empty_when_no_gets():
    spec = {"paths": {"/a": {"post": {}}, "/b": {"delete": {}}}}
    assert derive_get_urls(spec, "http://h") == []


@given(
    st.dictionaries(
        keys=st.from_regex(r"/[a-z]{1,8}", fullmatch=True),
        values=st.fixed_dictionaries({"get": st.just({})}),
        max_size=6,
    )
)
def test_derive_get_urls_never_leaves_placeholders(paths):
    # An emitted URL must never carry an unresolved `{param}` — those paths are
    # dropped, not shipped half-filled.
    urls = derive_get_urls({"paths": paths}, "http://h")
    assert all("{" not in u for u in urls)
