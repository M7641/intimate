WITH supply_wgts AS (
    SELECT
        created_at,
        SUM(supply_wgt) AS supply_wgt
    FROM (
        SELECT DISTINCT
            created_at,
            supply_wgt
        FROM {schema}.weekly_optimiser_output
        WHERE 1=1
        AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
        AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
        AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
        AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
    )
    GROUP BY created_at
),
demand_wgts AS (
    SELECT
        created_at,
        SUM(demand_wgt) AS demand_wgt
    FROM (
        SELECT DISTINCT
            created_at,
            demand_id,
            demand_wgt
        FROM {schema}.weekly_optimiser_output
        WHERE 1=1
        AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
        AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
        AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
        AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
    )
    GROUP BY created_at
),
allocated_wgts AS (
    SELECT
        created_at,
        SUM(allocated_wgt) AS allocated_wgt
    FROM {schema}.weekly_optimiser_output
    WHERE 1=1
    AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
    AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
    AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
    AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
    GROUP BY created_at
)
SELECT
    s.created_at,
    s.supply_wgt,
    d.demand_wgt,
    a.allocated_wgt
FROM supply_wgts s
JOIN demand_wgts d ON s.created_at = d.created_at
JOIN allocated_wgts a ON s.created_at = a.created_at
ORDER BY s.created_at
