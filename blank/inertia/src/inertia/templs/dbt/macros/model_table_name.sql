

{% macro model_table_name(table_name) %}

{%- if target.name in ['user', 'dev'] -%}
    SANDPIT.{{ table_name }}
{%- elif target.name == 'test' -%}
    TRANSFORM.{{ table_name }}
{%- elif target.name == 'prod' -%}
    PUBLISH.{{ table_name }}
{%- endif -%}

{% endmacro %}
