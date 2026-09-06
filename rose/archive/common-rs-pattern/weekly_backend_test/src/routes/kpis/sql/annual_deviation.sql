-- Compares the most recent weekly optimiser run per ISO week against the
-- annual plan total for the same ISO week. The weekly side bucket-uses
-- `EXTRACT(week FROM created_at)` because the persisted output does not
-- carry the target delivery week — runs are weekly, so the run's ISO
-- week is the closest available proxy. `weekly_annual_plan_v2` is
-- already keyed on `allocation_week`, so the join is a direct equality.
--
-- `created_at` on `weekly_optimiser_output_history` is VARCHAR (legacy
-- drift; see the schema-drift note in `save_outputs_to_db.py`), so it
-- must be cast to TIMESTAMP before EXTRACT can read a week from it.
WITH latest_run_per_week AS (
    SELECT
        EXTRACT(week FROM created_at::timestamp)::int AS iso_week,
        MAX(created_at) AS latest_created_at
    FROM {schema}.weekly_optimiser_output_history
    GROUP BY EXTRACT(week FROM created_at::timestamp)::int
),
weekly_totals AS (
    SELECT
        EXTRACT(week FROM o.created_at::timestamp)::int AS iso_week,
        SUM(o.allocated_wgt) AS weekly_wgt
    FROM {schema}.weekly_optimiser_output_history o
    JOIN latest_run_per_week l
      ON o.created_at = l.latest_created_at
    WHERE 1=1
    AND (%(mascode)s::varchar IS NULL OR o.mascode = %(mascode)s)
    AND (%(supcode)s::varchar IS NULL OR o.supcode = %(supcode)s)
    AND (%(hocustcode)s::varchar IS NULL OR o.hocustcode = %(hocustcode)s)
    GROUP BY EXTRACT(week FROM o.created_at::timestamp)::int
),
annual_totals AS (
    SELECT
        allocation_week AS iso_week,
        SUM(allocated_wgt) AS annual_wgt
    FROM {schema}.weekly_annual_plan_v2
    WHERE 1=1
    AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
    AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
    AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
    GROUP BY allocation_week
)
SELECT
    COALESCE(w.iso_week, a.iso_week) AS iso_week,
    COALESCE(w.weekly_wgt, 0)::float8 AS weekly_wgt,
    COALESCE(a.annual_wgt, 0)::float8 AS annual_wgt
FROM weekly_totals w
FULL OUTER JOIN annual_totals a
  ON w.iso_week = a.iso_week
ORDER BY iso_week
