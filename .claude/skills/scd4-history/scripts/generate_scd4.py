#!/usr/bin/env python3
"""Generate SCD Type 4 (history-table) DDL + load SQL for one dimension table.

SCD Type 4 keeps the *current* row in the main dimension table and every
*version* (current + superseded) in a sibling `<table>_history` table. Changes
are captured by a batch load that compares an incoming staging snapshot against
the current dimension, closes out the open history row for changed keys, inserts
the new version, then upserts the current row.

The emitted SQL is written in "SQL pur" — no MERGE, no triggers, no session
variables — so it runs on Redshift (a PG 8.0.2 fork) as well as modern Postgres.
The Redshift-idiomatic upsert is DELETE-then-INSERT inside one transaction.

Stdlib only. Reads a JSON config, writes two .sql files.

Usage:
    python generate_scd4.py --config table.json --out-dir ./sql
    python generate_scd4.py --config table.json --full-snapshot   # also close out vanished keys
    python generate_scd4.py --config table.json --print           # to stdout

Config schema (see assets/example_config.json):
    {
      "schema": "analytics",
      "table": "dim_customer",
      "business_key":    [{"name": "customer_id", "type": "BIGINT"}],   # identity; never versioned
      "tracked_columns": [{"name": "email", "type": "VARCHAR(255)"}],   # a change here = a new version
      "static_columns":  [{"name": "signup_date", "type": "DATE"}],     # carried, but never trigger a version
      "history_suffix": "_history",       # optional
      "staging_prefix": "stg_",           # optional
      "load_ts_placeholder": "{load_ts}"  # optional; substitute ONE run timestamp
    }
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# A rare delimiter for the change-detection hash. Plain '|' would collide when a
# value legitimately contains '|' (e.g. "a" + "|b" vs "a|" + "b"). chr(1) cannot
# appear in normal text, so the concatenation stays injective over the columns.
HASH_DELIM = "chr(1)"


def q(col: str) -> str:
    """Quote an identifier defensively (handles reserved words like `user`, `tier`)."""
    return f'"{col}"'


def fq(schema: str, table: str) -> str:
    return f"{q(schema)}.{q(table)}"


def names(cols: list[dict]) -> list[str]:
    return [c["name"] for c in cols]


def hash_expr(col_names: list[str], alias: str) -> str:
    """MD5 over the tracked columns, NULL-safe and delimiter-separated.

    Each column is cast to VARCHAR and NVL'd so a NULL never swallows the
    surrounding values, and the chr(1) separator keeps the concatenation
    injective. The identical expression is built for both staging and the
    dimension so the comparison is apples-to-apples.
    """
    parts = [f"NVL({alias}.{q(c)}::VARCHAR, '')" for c in col_names]
    return f"MD5({(' || ' + HASH_DELIM + ' || ').join(parts)})"


def load_config(path: Path) -> dict:
    cfg = json.loads(path.read_text())
    for required in ("schema", "table", "business_key", "tracked_columns"):
        if not cfg.get(required):
            sys.exit(f"config error: missing or empty '{required}'")
    cfg.setdefault("static_columns", [])
    cfg.setdefault("history_suffix", "_history")
    cfg.setdefault("staging_prefix", "stg_")
    cfg.setdefault("load_ts_placeholder", "{load_ts}")
    # tolerate a bare list of strings for business_key (default type BIGINT)
    cfg["business_key"] = [
        c if isinstance(c, dict) else {"name": c, "type": "BIGINT"}
        for c in cfg["business_key"]
    ]
    return cfg


def render_ddl(cfg: dict) -> str:
    schema, table = cfg["schema"], cfg["table"]
    hist = table + cfg["history_suffix"]
    bk = cfg["business_key"]
    all_cols = bk + cfg["tracked_columns"] + cfg["static_columns"]

    lines = [
        f"-- SCD Type 4 history table for {fq(schema, table)}",
        "-- Holds every version (current + superseded). is_current = TRUE marks the live row.",
        f"CREATE TABLE IF NOT EXISTS {fq(schema, hist)} (",
        "    scd_id        BIGINT IDENTITY(1,1),   -- surrogate version key",
    ]
    for c in all_cols:
        # business keys are NOT NULL (identity); payload columns stay nullable
        lines.append(
            f"    {q(c['name']):<13} {c['type']}{' NOT NULL' if c in bk else ''},"
        )
    lines += [
        "    row_hash      CHAR(32)  NOT NULL,   -- MD5 over tracked columns",
        "    valid_from    TIMESTAMP NOT NULL,",
        "    valid_to      TIMESTAMP NOT NULL DEFAULT '9999-12-31',  -- open-ended for the live row",
        "    is_current    BOOLEAN   NOT NULL DEFAULT TRUE,",
        "    PRIMARY KEY (scd_id)",
        ")",
        f"DISTKEY({q(bk[0]['name'])})",
        f"SORTKEY({', '.join(q(c['name']) for c in bk)}, valid_from);",
    ]
    return "\n".join(lines) + "\n"


def render_load(cfg: dict, full_snapshot: bool) -> str:
    schema, table = cfg["schema"], cfg["table"]
    hist = table + cfg["history_suffix"]
    stg = cfg["staging_prefix"] + table
    ts = cfg["load_ts_placeholder"]

    bk = names(cfg["business_key"])
    tracked = names(cfg["tracked_columns"])
    all_cols = bk + tracked + names(cfg["static_columns"])

    join_on = " AND ".join(f"s.{q(k)} = c.{q(k)}" for k in bk)
    bk_null = " OR ".join(f"c.{q(k)} IS NULL" for k in bk)
    sel_cols = ", ".join(f"s.{q(c)}" for c in all_cols)
    ins_cols = ", ".join(q(c) for c in all_cols)

    def key_match(left: str, right_alias: str) -> str:
        return " AND ".join(f"{left}.{q(k)} = {right_alias}.{q(k)}" for k in bk)

    s = [
        f"-- SCD Type 4 batch load: {fq(schema, stg)}  ->  history + current dimension",
        f"-- Substitute {ts} with ONE timestamp captured at the start of the run",
        "-- (e.g. dbt's run_started_at, Airflow's logical date). Do NOT use GETDATE() per",
        "-- statement — every version written in a batch must share the same boundary instant,",
        "-- otherwise valid_to of the old row won't exactly meet valid_from of the new one.",
        "BEGIN;",
        "",
        "-- 1. Detect new + changed business keys (hash diff over tracked columns).",
        "CREATE TEMP TABLE _scd_changed AS",
        f"SELECT {sel_cols},",
        f"       {hash_expr(tracked, 's')} AS row_hash",
        f"FROM {fq(schema, stg)} s",
        f"LEFT JOIN {fq(schema, table)} c ON {join_on}",
        f"WHERE ({bk_null})                                   -- brand-new key",
        f"   OR {hash_expr(tracked, 's')}",
        f"      <> {hash_expr(tracked, 'c')};                 -- a tracked attribute changed",
        "",
        "-- 2. Close out the open history row for every changed key.",
        f"UPDATE {fq(schema, hist)}",
        f"SET valid_to = '{ts}', is_current = FALSE",
        "FROM _scd_changed c",
        f"WHERE {fq(schema, hist)}.is_current = TRUE",
        f"  AND {key_match(fq(schema, hist), 'c')};",
        "",
        "-- 3. Insert the new version into history (open-ended, current).",
        f"INSERT INTO {fq(schema, hist)} ({ins_cols}, row_hash, valid_from, valid_to, is_current)",
        f"SELECT {ins_cols}, row_hash, '{ts}', '9999-12-31', TRUE",
        "FROM _scd_changed;",
        "",
        "-- 4. Upsert the current dimension (Redshift idiom: delete changed keys, re-insert).",
        f"DELETE FROM {fq(schema, table)}",
        "USING _scd_changed c",
        f"WHERE {key_match(fq(schema, table), 'c')};",
        "",
        f"INSERT INTO {fq(schema, table)} ({ins_cols})",
        f"SELECT {ins_cols}",
        "FROM _scd_changed;",
        "",
    ]
    if full_snapshot:
        s += [
            "-- 5. Full-snapshot mode: close out keys that vanished from staging (hard delete).",
            "--    Skip this block if staging carries deltas rather than a full snapshot.",
            f"UPDATE {fq(schema, hist)}",
            f"SET valid_to = '{ts}', is_current = FALSE",
            f"WHERE {fq(schema, hist)}.is_current = TRUE",
            "  AND NOT EXISTS (",
            f"      SELECT 1 FROM {fq(schema, stg)} s",
            f"      WHERE {key_match(fq(schema, hist), 's')});",
            "",
            f"DELETE FROM {fq(schema, table)}",
            "WHERE NOT EXISTS (",
            f"    SELECT 1 FROM {fq(schema, stg)} s",
            f"    WHERE {key_match(fq(schema, table), 's')});",
            "",
        ]
    s += ["DROP TABLE _scd_changed;", "COMMIT;"]
    return "\n".join(s) + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(
        description="Generate SCD Type 4 history DDL + load SQL."
    )
    ap.add_argument("--config", required=True, type=Path, help="JSON table config")
    ap.add_argument(
        "--out-dir", type=Path, default=Path("."), help="where to write the .sql files"
    )
    ap.add_argument(
        "--full-snapshot",
        action="store_true",
        help="staging is a full snapshot: also close out vanished keys (hard deletes)",
    )
    ap.add_argument(
        "--print", action="store_true", help="print to stdout instead of writing files"
    )
    args = ap.parse_args()

    cfg = load_config(args.config)
    ddl = render_ddl(cfg)
    load = render_load(cfg, args.full_snapshot)

    if args.print:
        print(ddl)
        print(load)
        return

    args.out_dir.mkdir(parents=True, exist_ok=True)
    table = cfg["table"]
    ddl_path = args.out_dir / f"{table}{cfg['history_suffix']}.ddl.sql"
    load_path = args.out_dir / f"scd4_load_{table}.sql"
    ddl_path.write_text(ddl)
    load_path.write_text(load)
    print(f"wrote {ddl_path}")
    print(f"wrote {load_path}")


if __name__ == "__main__":
    main()
