WITH customer_overview_stats AS (
    SELECT
        CAST(created_at AS DATE) AS created_at,
        MIN(allocated_wgt / demand_wgt) AS min_service_level,
        MAX(allocated_wgt / demand_wgt) AS max_service_level,
        AVG(allocated_wgt / demand_wgt) AS average_service_level,
        STDDEV(allocated_wgt / demand_wgt) AS std_dev_service_level
    FROM {schema}.weekly_optimiser_output
    WHERE 1=1
    AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
    AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
    AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
    AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
    GROUP BY CAST(created_at AS DATE)
)
SELECT * FROM customer_overview_stats
