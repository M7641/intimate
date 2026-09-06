WITH supplier_allocations AS (
    SELECT
        created_at,
        supcode,
        COUNT(DISTINCT CONCAT(hocustcode::varchar, brand::varchar)) as supplier_allocations
    FROM {schema}.weekly_optimiser_output
    WHERE 1=1
        AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
        AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
        AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
        AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
        AND allocated_wgt > 0
    GROUP BY created_at, supcode
),
supplier_overview_plot_df AS (
    SELECT
        CAST(created_at AS DATE) as created_at,
        MIN(supplier_allocations) as min_complexity,
        MAX(supplier_allocations) as max_complexity,
        AVG(supplier_allocations)::float as average_complexity
    FROM supplier_allocations
    GROUP BY created_at
    ORDER BY created_at
)
SELECT * FROM supplier_overview_plot_df;
