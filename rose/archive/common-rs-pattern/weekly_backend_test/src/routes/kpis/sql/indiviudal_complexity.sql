WITH supplier_allocations AS (
    SELECT
        created_at,
        supcode,
        COUNT(DISTINCT CONCAT(hocustcode, brand)) as supplier_allocations
    FROM {schema}.weekly_optimiser_output
    WHERE mascode = %(mascode)s
      AND allocated_wgt > 0
    GROUP BY created_at, supcode
)
SELECT
    created_at,
    supcode,
    supplier_allocations
FROM supplier_allocations
ORDER BY created_at
