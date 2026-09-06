"""The warehouse-backed extraction cache, driven by an injected fake DB.

Only the cache's own logic is tested — load filtering, JSON round-tripping,
fresh-only writes. The real DDL/SQL execution needs a warehouse and is
exercised by the deployed workflow, like the rest of output.py.
"""

from __future__ import annotations

import json

import polars as pl

from extract.output import (
    WarehouseExtractionCache,
    load_projection,
    save_projection,
    stored_embedding_models,
    write_embeddings,
)


class FakeDB:
    """Captures statements and serves canned rows — no warehouse."""

    def __init__(self, rows: list[dict] | None = None, tables_exist: bool = True):
        self.rows = rows or []  # canned load_data result rows
        self.tables_exist = tables_exist
        self.executed: list[str] = []
        self.written: list[tuple[pl.DataFrame, str, str]] = []
        self.load_params: dict | None = None

    def execute_query(self, query, params=None, log_query=False):
        self.executed.append(query)

    def load_data(self, query, params=None, **kwargs):
        self.load_params = params
        return pl.DataFrame(self.rows)

    def write_data(self, df, table, schema):
        self.written.append((df, table, schema))

    def table_exists(self, table, schema):
        return self.tables_exist


def build(
    rows=None, model_id="m", schema_domain="clothing"
) -> WarehouseExtractionCache:
    return WarehouseExtractionCache(
        FakeDB(rows), model_id=model_id, schema_domain=schema_domain
    )


def test_constructor_creates_the_table_and_scopes_the_load():
    cache = build()
    assert any("create table if not exists" in q for q in cache.db.executed)
    assert cache.db.load_params["model"] == "m"
    assert cache.db.load_params["domain"] == "clothing"


def test_sql_quotes_in_identifiers_are_doubled():
    cache = build(model_id="o'model", schema_domain="d'omain")
    assert cache.db.load_params["model"] == "o''model"
    assert cache.db.load_params["domain"] == "d''omain"


def test_get_parses_cached_features():
    rows = [{"key": "k", "features": '{"category": "dress"}'}]
    cache = build(rows)
    assert cache.get("k") == {"category": "dress"}
    assert cache.get("missing") is None


def test_a_malformed_row_does_not_poison_the_cache():
    rows = [
        {"key": "bad", "features": "{torn"},
        {"key": "good", "features": '{"a": 1}'},
    ]
    cache = build(rows)
    assert cache.get("bad") is None
    assert cache.get("good") == {"a": 1}


def test_put_many_writes_only_fresh_successful_entries():
    cache = build([{"key": "known", "features": '{"a": 1}'}])
    cache.put_many(
        {
            "known": {"a": 2},  # already cached → skipped
            "fresh": {"category": "dress"},
            "failed": {},  # failure → never stored, stays retryable
        }
    )
    ((df, table, schema),) = cache.db.written
    assert df.get_column("key").to_list() == ["fresh"]
    assert json.loads(df.get_column("features")[0]) == {"category": "dress"}
    assert (table, schema) == ("llm_extraction_cache_proto", "sandpit")
    # The fresh entry is served from memory afterwards.
    assert cache.get("fresh") == {"category": "dress"}


def test_put_many_with_nothing_fresh_writes_nothing():
    cache = build([{"key": "known", "features": '{"a": 1}'}])
    cache.put_many({"known": {"a": 1}, "failed": {}})
    assert cache.db.written == []


def test_projection_roundtrips_through_json():
    projection = {"dim": 2, "mean": [0.1, 0.2], "components": [[1.0, 0.0], [0.0, 1.0]]}
    db = FakeDB()
    save_projection(db, projection, model_id="enc")
    ((df, table, _),) = db.written
    assert table == "llm_embedding_projection_proto"
    assert df.get_column("dim").to_list() == [2]

    stored = FakeDB([{"projection": df.get_column("projection")[0]}])
    assert load_projection(stored, "enc", 2) == projection
    assert stored.load_params["model"] == "enc"


def test_load_projection_is_none_on_a_fresh_warehouse():
    assert load_projection(FakeDB(tables_exist=False), "enc", 2) is None
    assert load_projection(FakeDB([]), "enc", 2) is None


def test_write_embeddings_json_encodes_vectors():
    db = FakeDB()
    df = pl.DataFrame({"id": [1, 2], "embedding": [[0.1, 0.2], [0.3, 0.4]]})
    written = write_embeddings(db, df, model_id="enc", dim=2)
    assert written == 2
    ((out, table, _),) = db.written
    assert table == "llm_product_embedding_proto"
    assert out.get_column("id").to_list() == ["1", "2"]  # cast to varchar
    assert json.loads(out.get_column("embedding")[0]) == [0.1, 0.2]
    assert out.get_column("model").to_list() == ["enc", "enc"]


def test_write_embeddings_empty_frame_writes_nothing():
    db = FakeDB()
    empty = pl.DataFrame({"id": [], "embedding": []})
    assert write_embeddings(db, empty, model_id="enc", dim=2) == 0
    assert db.written == []


def test_stored_embedding_models_lists_distinct_encoders():
    db = FakeDB([{"model": "enc-b"}, {"model": "enc-a"}])
    assert stored_embedding_models(db) == ["enc-a", "enc-b"]
    assert stored_embedding_models(FakeDB(tables_exist=False)) == []
    assert stored_embedding_models(FakeDB([])) == []
