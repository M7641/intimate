WITH date_weeks AS (
    SELECT DISTINCT created_at
    FROM {schema}.weekly_optimiser_output
    WHERE 1=1
    AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
    AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
    AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
    AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
    ORDER BY created_at
),
cumulative_complexity AS (
    SELECT
        dw.created_at,
        COUNT(DISTINCT o.supcode || o.hocustcode || o.brand) as complexity_for_week
    FROM date_weeks dw
    CROSS JOIN {schema}.weekly_optimiser_output o
    WHERE 1=1
        AND (%(mascode)s::varchar IS NULL OR o.mascode = %(mascode)s)
        AND (%(supcode)s::varchar IS NULL OR o.supcode = %(supcode)s)
        AND (%(hocustcode)s::varchar IS NULL OR o.hocustcode = %(hocustcode)s)
        AND (%(created_at)s::varchar IS NULL OR o.created_at = %(created_at)s)
        AND o.allocated_wgt > 0
        AND o.created_at <= dw.created_at
    GROUP BY dw.created_at
)
SELECT
    created_at,
    complexity_for_week
FROM cumulative_complexity
ORDER BY created_at;
