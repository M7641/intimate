-- Patch Postgres toward the Redshift dialect the app's SQL assumes.
--
-- Redshift (and Snowflake) provide a MEDIAN() aggregate; stock Postgres does
-- NOT, so the column_stats numeric query (`MEDIAN("col"::FLOAT8)`) would 500
-- against an unpatched container. We define an equivalent aggregate over
-- `double precision` so the same query runs unchanged.
--
-- Add further Redshift/Snowflake shims here as the app's SQL needs them.

CREATE OR REPLACE FUNCTION _final_median(double precision[])
    RETURNS double precision AS $$
    SELECT AVG(val)
    FROM (
        SELECT val
        FROM unnest($1) val
        ORDER BY 1
        LIMIT 2 - MOD(array_upper($1, 1), 2)
        OFFSET CEIL(array_upper($1, 1) / 2.0) - 1
    ) sub;
$$ LANGUAGE sql IMMUTABLE;

DROP AGGREGATE IF EXISTS median(double precision);
CREATE AGGREGATE median(double precision) (
    SFUNC = array_append,
    STYPE = double precision[],
    FINALFUNC = _final_median,
    INITCOND = '{}'
);
