SELECT
    COALESCE(supply_id, '') || ':' || COALESCE(demand_id, '') AS row_id,
    supply_id::varchar  AS supply_id,
    demand_id::varchar  AS demand_id,
    mascode,
    supcode,
    variety,
    hocustcode,
    prodnum,
    tier,
    brand,
    countsize,
    demand_wgt,
    demand_qty,
    supply_wgt,
    allocated_wgt,
    allocated_qty,
    is_preferred
FROM {schema}.weekly_optimiser_output_history
WHERE optimiser_run_id = %(run_id)s
