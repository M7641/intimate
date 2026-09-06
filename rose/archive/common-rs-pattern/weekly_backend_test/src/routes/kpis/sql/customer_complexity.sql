WITH customer_allocations AS (
    SELECT
        created_at,
        hocustcode,
        COUNT(DISTINCT supcode) as customer_allocations
    FROM {schema}.weekly_optimiser_output
    WHERE 1=1
      AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
      AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
      AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
      AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
      AND allocated_wgt > 0
    GROUP BY created_at, hocustcode
)
SELECT
    created_at,
    MIN(customer_allocations) as min_complexity,
    MAX(customer_allocations) as max_complexity,
    AVG(customer_allocations)::float as average_complexity
FROM customer_allocations
GROUP BY created_at
ORDER BY created_at;
