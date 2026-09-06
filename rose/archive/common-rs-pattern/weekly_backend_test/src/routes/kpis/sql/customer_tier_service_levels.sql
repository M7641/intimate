-- Customer service levels broken down by tier (PREMIUM/STANDARD/VALUE)
SELECT
  CAST(created_at AS DATE) AS created_at,
  SUM(CASE WHEN tier = 'PREMIUM' THEN allocated_wgt ELSE 0 END) /
    NULLIF(SUM(CASE WHEN tier = 'PREMIUM' THEN demand_wgt ELSE 0 END), 0) as premium_service_level,
  SUM(CASE WHEN tier = 'STANDARD' THEN allocated_wgt ELSE 0 END) /
    NULLIF(SUM(CASE WHEN tier = 'STANDARD' THEN demand_wgt ELSE 0 END), 0) as standard_service_level,
  SUM(CASE WHEN tier = 'VALUE' THEN allocated_wgt ELSE 0 END) /
    NULLIF(SUM(CASE WHEN tier = 'VALUE' THEN demand_wgt ELSE 0 END), 0) as value_service_level
FROM {schema}.weekly_optimiser_output
WHERE 1=1
AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
GROUP BY CAST(created_at AS DATE)
ORDER BY created_at
