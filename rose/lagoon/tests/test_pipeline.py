"""Integration tests: query both S3 and the Postgres warehouse via MiniStack."""

from __future__ import annotations

from lagoon.pipeline import (
    connect_warehouse,
    load_into_warehouse,
    provision_warehouse,
    query_s3,
    query_warehouse,
    stage_to_s3,
)

from .conftest import requires_docker


@requires_docker
def test_s3_roundtrip(fresh_ministack):
    """Stage objects in S3, then read the keys back."""
    stage_to_s3(fresh_ministack, bucket="staging")
    keys = query_s3(fresh_ministack, bucket="staging")
    assert keys == ["events/1.json", "events/2.json", "events/3.json"]


@requires_docker
def test_warehouse_aggregation(fresh_ministack):
    """Provision the warehouse, load events, and run an aggregation query."""
    params = provision_warehouse(fresh_ministack, identifier="wh-agg")
    with connect_warehouse(params) as conn:
        load_into_warehouse(conn)
        totals = query_warehouse(conn)
    # SAMPLE_EVENTS: emea = 120 + 50, amer = 80
    assert totals == [("amer", 80), ("emea", 170)]


@requires_docker
def test_both_stores_agree(fresh_ministack):
    """The number of staged S3 objects matches the rows loaded into the warehouse."""
    stage_to_s3(fresh_ministack, bucket="staging")
    params = provision_warehouse(fresh_ministack, identifier="wh-count")
    with connect_warehouse(params) as conn:
        load_into_warehouse(conn)
        with conn.cursor() as cur:
            cur.execute("SELECT COUNT(*) FROM events")
            (row_count,) = cur.fetchone()

    assert len(query_s3(fresh_ministack, bucket="staging")) == row_count
