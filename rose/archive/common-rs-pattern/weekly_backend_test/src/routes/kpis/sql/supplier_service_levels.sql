-- plot for Supplier Service Levels
WITH supply_wgts AS (
  SELECT
    created_at,
    supcode,
    SUM(supply_wgt) as supply_wgt
  FROM (
    SELECT DISTINCT
      created_at,
      supcode,
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
    SUM(allocated_wgt) as allocated_wgt
  FROM {schema}.weekly_optimiser_output
  WHERE 1=1
  AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
  AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
  AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
  AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
  GROUP BY created_at, supcode
),
supplier_overview AS (
  SELECT
    s.created_at,
    s.supcode,
    s.supply_wgt,
    a.allocated_wgt
  FROM supply_wgts s
  JOIN allocated_wgts a
    ON s.created_at = a.created_at
    AND s.supcode = a.supcode
),
supplier_overview_plot AS (
  SELECT
    created_at,
    MIN(allocated_wgt / supply_wgt) as min_service_level,
    MAX(allocated_wgt / supply_wgt) as max_service_level,
    AVG(allocated_wgt / supply_wgt) as average_service_level,
    STDDEV(allocated_wgt / supply_wgt) as std_dev_service_level
  FROM supplier_overview
  GROUP BY created_at
  ORDER BY created_at
)
SELECT * FROM supplier_overview_plot
