-- Integration-test fixtures for the data_view API.
--
-- Shape mirrors the production "snapshot" model the API assumes:
--   * a `load_timestamp` column marks each load of a table (multiple loads
--     coexist; the API filters to one, defaulting to the latest);
--   * a set of internal columns (row_hash, batch_group_id, snapshot_type,
--     source_system) the API deliberately hides from its surface;
--   * at least one table WITHOUT a load_timestamp, to exercise that branch.
--
-- Kept portable (no Postgres-only features) so the same SQL runs through the
-- Postgres, Redshift (wire-compatible) and Snowflake-mock backends unchanged.

CREATE SCHEMA IF NOT EXISTS stage;
CREATE SCHEMA IF NOT EXISTS marts;

-- Snapshot table: business columns + internal columns the API excludes.
CREATE TABLE stage.customer_snapshot (
    customer_id     integer,
    customer_name   varchar(200),
    region          varchar(50),
    lifetime_value  numeric(12, 2),
    orders_count    integer,
    is_active       boolean,
    -- internal columns — hidden by the API, used only for snapshot bookkeeping
    row_hash        varchar(64),
    load_timestamp  timestamp,
    batch_group_id  varchar(64),
    snapshot_type   varchar(20),
    source_system   varchar(50)
);

-- A second snapshot table, so schema/table listing returns more than one.
CREATE TABLE stage.order_snapshot (
    order_id        integer,
    customer_id     integer,
    amount          numeric(12, 2),
    status          varchar(30),
    load_timestamp  timestamp
);

-- Reference table with NO load_timestamp: the API must treat it as a single
-- unversioned dataset (no timestamp selector, no snapshot filtering).
CREATE TABLE marts.region_lookup (
    region          varchar(50),
    country         varchar(80),
    population      bigint
);
