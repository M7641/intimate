-- Customer summary metrics: number of SKUs, number of growers
SELECT
  CAST(created_at AS DATE) AS created_at,
  COUNT(DISTINCT countsize) as number_of_skus,
  COUNT(DISTINCT supcode) as number_of_growers
FROM {schema}.weekly_optimiser_output
WHERE 1=1
AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
GROUP BY CAST(created_at AS DATE)
ORDER BY created_at
