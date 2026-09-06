import datetime
import json

import polars as pl

from extract.database import DBActions


def init_table(
    db: DBActions,
    schema: str = "sandpit",
    table: str = "llm_product_hierarchy_proto",
    log_query: bool = False,
) -> None:
    """Create the output table if it does not exist (idempotent DDL)."""
    db.execute_query(
        query="""
            -- Output layer: the LLM-generated product_type ("the goods").
            --
            -- One row per source product. Idempotency is enforced upstream by an anti-join
            -- on `id` (we never re-generate an id we already stored), with `id` as the
            -- primary key as a backstop. See output_data_layer.dbml for the interface.
            --
            -- Rendered with Jinja: pass `schema` and `table`. Run via `extract init`.
            create table if not exists {{ schema }}.{{ table }} (
                id            varchar       not null,
                product_type  varchar,                  -- generated product type
                model         varchar,                  -- checkpoint that produced the row
                schema_domain varchar,                  -- extraction schema (e.g. "product type")
                created_at    timestamp_ntz not null default current_timestamp,
                updated_at    timestamp_ntz not null default current_timestamp,
                constraint pk_{{ table }} primary key (id)
            );
        """,
        params={"schema": schema, "table": table},
        log_query=log_query,
    )


def load_existing_ids(
    db: DBActions,
    schema: str = "sandpit",
    table: str = "llm_product_hierarchy_proto",
) -> pl.DataFrame:
    """Return a single-column (``id``: Utf8) frame of ids already stored.

    Empty when the table is absent or has no rows — so the first run treats
    every candidate as new.
    """
    empty = pl.DataFrame({"id": []}, schema={"id": pl.Utf8})
    if not db.table_exists(table, schema):
        return empty
    df = db.load_data(
        query='select id as "id" from {{ schema }}.{{ table }}',
        params={"schema": schema, "table": table},
    )
    if "id" not in df.columns:
        return empty
    return df.select(pl.col("id").cast(pl.Utf8))


def load_hierarchy(
    db: DBActions,
    schema: str = "sandpit",
    table: str = "llm_product_hierarchy_proto",
) -> pl.DataFrame:
    """Return the generated product_type values already stored.

    Empty (typed) frame when the table is absent — so taxonomy discovery on a
    fresh table just yields nothing rather than erroring.
    """
    cols = {"product_type": pl.Utf8}
    if not db.table_exists(table, schema):
        return pl.DataFrame(schema=cols)
    return db.load_data(
        query='select product_type as "product_type" from {{ schema }}.{{ table }}',
        params={"schema": schema, "table": table},
    )


def init_concept_table(
    db: DBActions,
    schema: str = "sandpit",
    table: str = "llm_hierarchy_concept_proto",
    log_query: bool = False,
) -> None:
    """Create the L2 concept-taxonomy table (idempotent DDL).

    Centroids are persisted (JSON float array) so condensing stays incremental:
    a run embeds only the new batch and assigns it against these vectors.
    """
    db.execute_query(
        query="""
            create table if not exists {{ schema }}.{{ table }} (
                concept_id  number        not null,
                level       varchar       not null,
                parent_id   number,                       -- parent concept (null at the top level)
                canonical   varchar,
                centroid    varchar,                      -- json array of floats (embedding)
                support     number        not null default 0,
                status      varchar       not null,       -- 'pending' until min-support, then 'active'
                created_at  timestamp_ntz not null default current_timestamp,
                updated_at  timestamp_ntz not null default current_timestamp,
                constraint pk_{{ table }} primary key (concept_id)
            );
        """,
        params={"schema": schema, "table": table},
        log_query=log_query,
    )


def init_final_table(
    db: DBActions,
    schema: str = "sandpit",
    table: str = "llm_product_hierarchy_final_proto",
    log_query: bool = False,
) -> None:
    """Create the L3 final table (idempotent DDL): per-product canonical label."""
    db.execute_query(
        query="""
            create table if not exists {{ schema }}.{{ table }} (
                id           varchar       not null,
                product_type varchar,
                created_at   timestamp_ntz not null default current_timestamp,
                updated_at   timestamp_ntz not null default current_timestamp,
                constraint pk_{{ table }} primary key (id)
            );
        """,
        params={"schema": schema, "table": table},
        log_query=log_query,
    )


def init_cache_table(
    db: DBActions,
    schema: str = "sandpit",
    table: str = "llm_extraction_cache_proto",
    log_query: bool = False,
) -> None:
    """Create the extraction-cache table (idempotent DDL).

    One row per (description key, model, schema): the validated feature dict,
    JSON-encoded — same storage move as the concept centroids. Workflow steps
    run in ephemeral containers, so this is the cache storage that actually
    persists between scheduled runs (Step 0 in README).
    """
    db.execute_query(
        query="""
            create table if not exists {{ schema }}.{{ table }} (
                key           varchar       not null,    -- sha256 of the normalised description
                model         varchar       not null,    -- checkpoint that produced the features
                schema_domain varchar       not null,    -- extraction schema (e.g. "generic")
                features      varchar       not null,    -- json-encoded feature dict
                created_at    timestamp_ntz not null default current_timestamp,
                constraint pk_{{ table }} primary key (key, model, schema_domain)
            );
        """,
        params={"schema": schema, "table": table},
        log_query=log_query,
    )


class WarehouseExtractionCache:
    """Snowflake-backed extraction cache — same get/put_many surface as
    :class:`extract.dedup.ExtractionCache`, so the pipeline takes
    either interchangeably.

    Entries for this run's (model, schema) pair load once up front; ``put_many``
    inserts only fresh, successful results. Failed extractions are never
    stored, so they stay retryable on the next run.
    """

    def __init__(
        self,
        db: DBActions,
        *,
        model_id: str,
        schema_domain: str,
        schema: str = "sandpit",
        table: str = "llm_extraction_cache_proto",
    ) -> None:
        init_cache_table(db, schema=schema, table=table)
        self.db = db
        self.model_id = model_id
        self.schema_domain = schema_domain
        self.schema = schema
        self.table = table
        self.entries = self._load()

    def get(self, key: str) -> dict | None:
        return self.entries.get(key)

    def put_many(self, features_by_key: dict[str, dict]) -> None:
        fresh = {
            key: feats
            for key, feats in features_by_key.items()
            if feats and key not in self.entries
        }
        if not fresh:
            return
        df = pl.DataFrame(
            {
                "key": list(fresh),
                "model": [self.model_id] * len(fresh),
                "schema_domain": [self.schema_domain] * len(fresh),
                "features": [json.dumps(f, ensure_ascii=False) for f in fresh.values()],
            }
        )
        self.db.write_data(df, self.table, self.schema)
        self.entries.update(fresh)

    def _load(self) -> dict[str, dict]:
        df = self.db.load_data(
            query='select key as "key", features as "features" '
            "from {{ schema }}.{{ table }} "
            "where model = '{{ model }}' and schema_domain = '{{ domain }}'",
            params={
                "schema": self.schema,
                "table": self.table,
                # Inline literals are jinja-rendered; double quotes the SQL way.
                "model": self.model_id.replace("'", "''"),
                "domain": self.schema_domain.replace("'", "''"),
            },
        )
        if "key" not in df.columns:
            return {}
        entries: dict[str, dict] = {}
        for r in df.iter_rows(named=True):
            try:
                entries[r["key"]] = json.loads(r["features"])
            except (TypeError, json.JSONDecodeError):
                continue  # a malformed row must not poison the whole cache
        return entries


def init_embedding_table(
    db: DBActions,
    schema: str = "sandpit",
    table: str = "llm_product_embedding_proto",
    log_query: bool = False,
) -> None:
    """Create the compressed-embedding table (idempotent DDL).

    One row per product: the PCA-compressed description embedding (A1 in
    README), JSON-encoded like the concept centroids. ``model``/``dim``
    record which encoder and projection produced the vector — vectors from
    different (model, dim) pairs are not comparable.
    """
    db.execute_query(
        query="""
            create table if not exists {{ schema }}.{{ table }} (
                id         varchar       not null,
                embedding  varchar       not null,    -- json array of floats (PCA-compressed)
                model      varchar       not null,    -- encoder checkpoint
                dim        number        not null,    -- compressed width
                created_at timestamp_ntz not null default current_timestamp,
                constraint pk_{{ table }} primary key (id)
            );
        """,
        params={"schema": schema, "table": table},
        log_query=log_query,
    )


def init_projection_table(
    db: DBActions,
    schema: str = "sandpit",
    table: str = "llm_embedding_projection_proto",
    log_query: bool = False,
) -> None:
    """Create the PCA-projection table (idempotent DDL).

    One row per (encoder, dim): the fitted projection (mean + components,
    JSON). Persisting it is what keeps every run in the SAME space — new
    products must be comparable to the embeddings already stored.
    """
    db.execute_query(
        query="""
            create table if not exists {{ schema }}.{{ table }} (
                model      varchar       not null,    -- encoder checkpoint
                dim        number        not null,    -- compressed width
                projection varchar       not null,    -- json {"dim", "mean", "components"}
                created_at timestamp_ntz not null default current_timestamp,
                constraint pk_{{ table }} primary key (model, dim)
            );
        """,
        params={"schema": schema, "table": table},
        log_query=log_query,
    )


def load_projection(
    db: DBActions,
    model_id: str,
    dim: int,
    schema: str = "sandpit",
    table: str = "llm_embedding_projection_proto",
) -> dict | None:
    """The stored PCA projection for (encoder, dim), or None on the first run."""
    if not db.table_exists(table, schema):
        return None
    df = db.load_data(
        query='select projection as "projection" '
        "from {{ schema }}.{{ table }} "
        "where model = '{{ model }}' and dim = {{ dim }}",
        params={
            "schema": schema,
            "table": table,
            "model": model_id.replace("'", "''"),
            "dim": int(dim),
        },
    )
    if "projection" not in df.columns or df.is_empty():
        return None
    return json.loads(df.get_column("projection")[0])


def save_projection(
    db: DBActions,
    projection: dict,
    model_id: str,
    schema: str = "sandpit",
    table: str = "llm_embedding_projection_proto",
) -> None:
    """Persist a freshly-fitted projection for (encoder, dim)."""
    df = pl.DataFrame(
        {
            "model": [model_id],
            "dim": [projection["dim"]],
            "projection": [json.dumps(projection, ensure_ascii=False)],
        }
    )
    db.write_data(df, table, schema)


def stored_embedding_models(
    db: DBActions,
    schema: str = "sandpit",
    table: str = "llm_product_embedding_proto",
) -> list[str]:
    """The distinct encoder checkpoints already in the embedding table.

    Vectors from different encoders live in incomparable spaces, and the
    anti-join on ``id`` would never re-embed old rows with a new encoder — so
    the CLI refuses to mix: switching encoders means clearing the table.
    """
    if not db.table_exists(table, schema):
        return []
    df = db.load_data(
        query='select distinct model as "model" from {{ schema }}.{{ table }}',
        params={"schema": schema, "table": table},
    )
    if "model" not in df.columns:
        return []
    return sorted(df.get_column("model").to_list())


def write_embeddings(
    db: DBActions,
    df: pl.DataFrame,
    model_id: str,
    dim: int,
    schema: str = "sandpit",
    table: str = "llm_product_embedding_proto",
) -> int:
    """Insert compressed embeddings (id + json vector). Returns rows written.

    Assumes ``df`` rows are new — the warehouse anti-join on ``id`` already
    removed products with a stored embedding, mirroring ``write_hierarchy``.
    """
    if df.is_empty():
        return 0
    insert = pl.DataFrame(
        {
            "id": df.get_column("id").cast(pl.Utf8),
            "embedding": [
                json.dumps(vector) for vector in df.get_column("embedding").to_list()
            ],
            "model": [model_id] * df.height,
            "dim": [dim] * df.height,
        }
    )
    db.write_data(insert, table, schema)
    return insert.height


def init_all(
    db: DBActions,
    *,
    schema: str = "sandpit",
    recreate: bool = False,
    log_query: bool = False,
) -> None:
    """Create every output table (idempotent): L1 / L2 / L3, the extraction
    cache, and the embedding + projection tables.

    ``recreate=True`` DROPs them first — needed after a generated-schema change,
    because ``create table if not exists`` cannot migrate an existing table's
    columns (e.g. switching segment/family/subtype → product_type). The cache
    is dropped too (its JSON rows embed the old schema's fields), and so are
    the embeddings with their projection (they only make sense together).
    """
    if recreate:
        for table in (
            "llm_product_hierarchy_proto",
            "llm_hierarchy_concept_proto",
            "llm_product_hierarchy_final_proto",
            "llm_extraction_cache_proto",
            "llm_product_embedding_proto",
            "llm_embedding_projection_proto",
        ):
            db.execute_query(
                query="drop table if exists {{ schema }}.{{ table }}",
                params={"schema": schema, "table": table},
                log_query=log_query,
            )
    init_table(db, schema=schema, log_query=log_query)
    init_concept_table(db, schema=schema, log_query=log_query)
    init_final_table(db, schema=schema, log_query=log_query)
    init_cache_table(db, schema=schema, log_query=log_query)
    init_embedding_table(db, schema=schema, log_query=log_query)
    init_projection_table(db, schema=schema, log_query=log_query)


def load_concepts(
    db: DBActions,
    schema: str = "sandpit",
    table: str = "llm_hierarchy_concept_proto",
) -> list[dict]:
    """Load the persisted concept tree as condense-ready dicts (centroids parsed)."""
    if not db.table_exists(table, schema):
        return []
    df = db.load_data(
        query='select concept_id as "concept_id", level as "level", '
        'parent_id as "parent_id", canonical as "canonical", '
        'centroid as "centroid", support as "support", status as "status" '
        "from {{ schema }}.{{ table }}",
        params={"schema": schema, "table": table},
    )
    concepts = []
    for r in df.iter_rows(named=True):
        concepts.append(
            {
                "concept_id": int(r["concept_id"]),
                "level": r["level"],
                "parent_id": None if r["parent_id"] is None else int(r["parent_id"]),
                "canonical": r["canonical"],
                "centroid": json.loads(r["centroid"]),
                "support": int(r["support"]),
                "status": r["status"],
            }
        )
    return concepts


def save_concepts(
    db: DBActions,
    concepts: list[dict],
    schema: str = "sandpit",
    table: str = "llm_hierarchy_concept_proto",
) -> None:
    """Upsert the concept tree by concept_id (preserves created_at)."""
    if not concepts:
        return
    df = pl.DataFrame(
        {
            "concept_id": [c["concept_id"] for c in concepts],
            "level": [c["level"] for c in concepts],
            "parent_id": [c["parent_id"] for c in concepts],
            "canonical": [c["canonical"] for c in concepts],
            "centroid": [json.dumps(c["centroid"]) for c in concepts],
            "support": [c["support"] for c in concepts],
            "status": [c["status"] for c in concepts],
        }
    )
    db.upsert_data(df, ["concept_id"], table, schema)


def load_unmapped(
    db: DBActions,
    schema: str = "sandpit",
    raw_table: str = "llm_product_hierarchy_proto",
    final_table: str = "llm_product_hierarchy_final_proto",
) -> pl.DataFrame:
    """Raw rows not yet in the final table — the new batch to condense."""
    cols = {"id": pl.Utf8, "product_type": pl.Utf8}
    if not db.table_exists(raw_table, schema):
        return pl.DataFrame(schema=cols)
    return db.load_data(
        query='select r.id as "id", r.product_type as "product_type" '
        "from {{ schema }}.{{ raw }} r "
        "left join {{ schema }}.{{ final }} f on r.id = f.id "
        "where f.id is null",
        params={"schema": schema, "raw": raw_table, "final": final_table},
    )


def write_final(
    db: DBActions,
    mapped: list[dict],
    schema: str = "sandpit",
    table: str = "llm_product_hierarchy_final_proto",
) -> int:
    """Insert mapped (canonical) rows into the final table. Returns rows written."""
    if not mapped:
        return 0
    df = pl.DataFrame(mapped).select("id", "product_type")
    db.write_data(df, table, schema)
    return df.height


def load_unallocated(
    db: DBActions,
    schema: str = "sandpit",
    raw_table: str = "llm_product_hierarchy_proto",
    final_table: str = "llm_product_hierarchy_final_proto",
) -> list[dict]:
    """Final rows still 'unallocated', carrying their original raw text.

    Each dict has ``id``, the raw ``product_type`` (to re-place against the
    current taxonomy), and ``cur_product_type`` (the current final label, to
    detect what actually changed).
    """
    if not db.table_exists(final_table, schema):
        return []
    df = db.load_data(
        query='select f.id as "id", r.product_type as "product_type", '
        'f.product_type as "cur_product_type" '
        "from {{ schema }}.{{ final }} f "
        "join {{ schema }}.{{ raw }} r on f.id = r.id "
        "where f.product_type = 'unallocated'",
        params={"schema": schema, "raw": raw_table, "final": final_table},
    )
    return df.to_dicts()


def update_final(
    db: DBActions,
    mapped: list[dict],
    schema: str = "sandpit",
    table: str = "llm_product_hierarchy_final_proto",
) -> int:
    """Upsert re-placed rows into the final table by id. Returns rows updated."""
    if not mapped:
        return 0
    df = pl.DataFrame(mapped).select("id", "product_type")
    db.upsert_data(df, ["id"], table, schema)
    return df.height


def filter_new(
    df: pl.DataFrame,
    existing_ids: pl.DataFrame,
    id_col: str = "id",
) -> pl.DataFrame:
    """Anti-join ``df`` against already-stored ids — keep only new rows.

    Casts the join key to Utf8 on both sides so an integer ``sku_id`` matches a
    varchar id in the table.
    """
    return df.with_columns(pl.col(id_col).cast(pl.Utf8)).join(
        existing_ids, on=id_col, how="anti"
    )


def write_hierarchy(
    db: DBActions,
    df: pl.DataFrame,
    model_id: str,
    schema_domain: str,
    schema: str = "sandpit",
    table: str = "llm_product_hierarchy_proto",
    feature_cols: tuple[str, ...] = ("product_type",),
    at: datetime.datetime | None = None,
) -> int:
    """Insert generated rows with timestamps; returns rows written.

    Assumes ``df`` rows are new (the anti-join already removed duplicates), so
    this is a plain insert: ``created_at`` and ``updated_at`` start equal. ``at``
    lets the caller pin the timestamp (kept TZ-naive for ``TIMESTAMP_NTZ``).
    """
    now = (at or datetime.datetime.now(datetime.timezone.utc)).replace(tzinfo=None)

    insert = (
        df.select(
            pl.col("id").cast(pl.Utf8),
            *(pl.col(c).cast(pl.Utf8) for c in feature_cols),
        )
        .with_columns(
            pl.lit(model_id).alias("model"),
            pl.lit(schema_domain).alias("schema_domain"),
            pl.lit(now).alias("created_at"),
            pl.lit(now).alias("updated_at"),
        )
        .select(
            "id",
            *feature_cols,
            "model",
            "schema_domain",
            "created_at",
            "updated_at",
        )
    )
    db.write_data(insert, table, schema)
    return insert.height
