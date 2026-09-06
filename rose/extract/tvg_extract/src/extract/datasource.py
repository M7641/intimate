import polars as pl

from extract.database import DBActions


def load_products(
    sample_size: int | None = 50,
    exclude_schema: str | None = None,
    exclude_table: str | None = None,
    division: str | None = None,
    department: str | None = None,
) -> pl.DataFrame:
    """Load a sampled, pipeline-ready product frame from the warehouse.

    Returns a frame with exactly ``id`` / ``description``.

    ``division`` restricts the run to a single ``product_division`` (e.g.
    "Womenswear"); ``department`` restricts to a single ``product_department``
    (Taxonomy L2, e.g. "FURNITURE"). Both filters are optional and AND together
    when supplied. When ``exclude_table`` is given, ids already present in
    ``exclude_schema.exclude_table`` are filtered out **in the warehouse query**
    (an anti-join before the sample), so a run only pulls — and ``sample_size``
    only counts — genuinely new products.

    ``sample_size=None`` pulls **every** matching product — the whole (filtered)
    category, in natural order. The random-order sample is dropped: scanning a
    full category, a global ``order by random()`` is a pure-overhead sort, and
    there is nothing to cap.
    """
    actions = DBActions()

    try:
        df = actions.load_data(
            query="""
            with hierarchy as (
                select
                    "product_id" as product_id,
                    "sku_id" as sku_id,
                    "product_name" as product_name,
                    "heading_text" as heading_text,
                    "product_copy" as product_copy
                from stage.prd1_product_hierarchy
                -- Reduce rows: launched products whose exit date has passed.
                -- NOTE: assumes these date columns live on prd1_product_hierarchy.
                where "actual_exit_date"::date < current_date::date
                  and "actual_launch_date" is not null
                  {% if division %}
                  -- Restrict the run to one product_division.
                  and "product_division" = '{{ division }}'
                  {% endif %}
                  {% if department %}
                  -- Restrict the run to one product_department (Taxonomy L2).
                  and "product_department" = '{{ department }}'
                  {% endif %}
            ),
            attributes as (
                select
                    "sku_id" as sku_id,
                    "attribute_details" as attribute_details,
                    "navaigation_attributes" as navaigation_attributes
                from stage.prd1_product_attributes
            ),
            joined as (
                select
                    h.product_id,
                    h.product_name,
                    h.heading_text,
                    h.product_copy,
                    a.attribute_details,
                    a.navaigation_attributes
                from hierarchy h
                left join attributes a
                       on a.sku_id = h.sku_id
                -- Collapse SKU variants to one representative row per product.
                qualify row_number() over (
                    partition by h.product_id order by h.sku_id
                ) = 1
            ),
            shaped as (
                select
                    product_id as id,
                    -- Build from an array, not concat_ws: array_construct_compact
                    -- drops NULL elements, so a single NULL column can never null
                    -- the whole description. nullif(trim(x), '') turns blank /
                    -- whitespace-only fields into NULLs so they drop out too.
                    -- A row with every text field empty yields '' (not NULL).
                    array_to_string(
                        array_construct_compact(
                            nullif(trim(product_name), ''),
                            nullif(trim(heading_text), ''),
                            nullif(trim(product_copy), ''),
                            nullif(trim(to_varchar(attribute_details)), ''),
                            nullif(trim(to_varchar(navaigation_attributes)), '')
                        ),
                        chr(10)
                    ) as description
                from joined
            )
            select
                id as "id",
                description as "description"
            from shaped
            where description is not null
              and trim(description) <> ''
              {% if exclude_table %}
              -- Skip ids already stored: anti-join in-warehouse, before the
              -- sample, so we only pull (and only count) new products.
              and not exists (
                  select 1 from {{ exclude_schema }}.{{ exclude_table }} e
                  where e.id = cast(shaped.id as varchar)
              )
              {% endif %}
            {% if sample_size %}
            -- A random sample, capped. Omitted for a whole-category run
            -- (sample_size=None): we want every row, and a global sort over
            -- the full category would be wasted work.
            order by random()
            limit {{ sample_size }}
            {% endif %}
            """,
            params={
                "sample_size": int(sample_size) if sample_size else None,
                "exclude_schema": exclude_schema,
                "exclude_table": exclude_table,
                "division": division,
                "department": department,
            },
        )
    finally:
        actions.close()

    df = df.rename({c: c.lower() for c in df.columns})

    missing = {"id", "description"} - set(df.columns)

    if missing:
        raise ValueError(f"query did not return expected columns: {sorted(missing)}")

    return df.select(
        pl.col("id"),
        pl.col("description").cast(pl.Utf8),
    )
