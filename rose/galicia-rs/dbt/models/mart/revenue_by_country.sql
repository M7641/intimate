-- Revenu par pays. Le SQL est exactement celui de `pipeline::build_mart` côté
-- Rust (constante MART_SQL) : `galicia mart` et `dbt run` produisent la même
-- table, à partir des mêmes Parquet. C'est le point du PoC — le SQL est
-- portable, seul le câblage source→fichiers change.
{{ config(materialized='table') }}

select
    c.country,
    count(*) filter (where o.status = 'paid')                      as paid_orders,
    sum(o.amount_cents) filter (where o.status = 'paid') / 100.0   as gross_revenue_eur,
    coalesce(sum(o.discount_cents), 0) / 100.0                     as total_discount_eur
from {{ source('raw', 'orders') }} o
join {{ source('raw', 'customers') }} c using (customer_id)
group by c.country
order by gross_revenue_eur desc
