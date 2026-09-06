-- Supplier service levels broken down by tier (PREMIUM/STANDARD/VALUE)
WITH supply_wgts AS (
  SELECT
    created_at,
    supcode,
    SUM(CASE WHEN tier = 'PREMIUM' THEN supply_wgt ELSE 0 END) as premium_supply,
    SUM(CASE WHEN tier = 'STANDARD' THEN supply_wgt ELSE 0 END) as standard_supply,
    SUM(CASE WHEN tier = 'VALUE' THEN supply_wgt ELSE 0 END) as value_supply
  FROM (
    SELECT DISTINCT
      created_at,
      supcode,
      tier,
      supply_wgt
    FROM {schema}.weekly_optimiser_output
    WHERE 1=1
    AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
    AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
    AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
    AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
  ) distinct_supply
  GROUP BY created_at, supcode
),
allocated_wgts AS (
  SELECT
    created_at,
    supcode,
    SUM(CASE WHEN tier = 'PREMIUM' THEN allocated_wgt ELSE 0 END) as premium_alloc,
    SUM(CASE WHEN tier = 'STANDARD' THEN allocated_wgt ELSE 0 END) as standard_alloc,
    SUM(CASE WHEN tier = 'VALUE' THEN allocated_wgt ELSE 0 END) as value_alloc
  FROM {schema}.weekly_optimiser_output
  WHERE 1=1
  AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
  AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
  AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
  AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
  GROUP BY created_at, supcode
)
SELECT
  CAST(s.created_at AS DATE) AS created_at,
  SUM(a.premium_alloc) / NULLIF(SUM(s.premium_supply), 0) as premium_service_level,
  SUM(a.standard_alloc) / NULLIF(SUM(s.standard_supply), 0) as standard_service_level,
  SUM(a.value_alloc) / NULLIF(SUM(s.value_supply), 0) as value_service_level
FROM supply_wgts s
JOIN allocated_wgts a
  ON s.created_at = a.created_at
  AND s.supcode = a.supcode
GROUP BY s.created_at
ORDER BY s.created_at
