-- This macro exploits the dbt aliasing mechanism to alter the destination
-- name of a model in the case where `target == 'user'`. The outcome is
-- a table prefixed by $USER which allows multiple users to develop
-- in the same schema.
{% macro generate_alias_name(custom_alias_name=none, node=none) -%}
    {%- if custom_alias_name is none -%}
        {%- set table_name = node.name -%}
    {%- else -%}
        {%- set table_name = custom_alias_name | trim -%}
    {%- endif -%}

    {%- if target.name == 'user' -%}
        {{ env_var('USER') | lower ~ '_' ~ table_name }}
    {%- else -%}
        {{ table_name }}
    {%- endif -%}
{%- endmacro %}
