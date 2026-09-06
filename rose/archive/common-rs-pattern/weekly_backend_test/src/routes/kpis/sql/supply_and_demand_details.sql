WITH supply_wgts AS (
    SELECT created_at, SUM(supply_wgt) AS supply_wgt
    FROM (
        SELECT DISTINCT created_at, supply_wgt
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
    SELECT created_at, SUM(demand_wgt) AS demand_wgt
    FROM (
        SELECT DISTINCT created_at, demand_id, demand_wgt
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
    SELECT created_at, SUM(allocated_wgt) AS allocated_wgt
    FROM {schema}.weekly_optimiser_output
    WHERE 1=1
    AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
    AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
    AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
    AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
    GROUP BY created_at
)
SELECT created_at, 'Supply Weight' AS metric, ROUND(supply_wgt, 0) AS value FROM supply_wgts
UNION ALL
SELECT created_at, 'Demand Weight' AS metric, ROUND(demand_wgt, 0) AS value FROM demand_wgts
UNION ALL
SELECT created_at, 'Allocated Weight' AS metric, ROUND(allocated_wgt, 0) AS value FROM allocated_wgts
ORDER BY created_at, metric
