{% macro validate_column_type(model, column_name, column_types) %}
    {%- set column_name = column_name | upper -%}
    {%- set columns_in_relation = adapter.get_columns_in_relation(model) -%}
    {%- set column_type_list = column_type_list| map("upper") | list -%}

    with

    relation_columns as (
        {% for column in columns_in_relation %}
        select
            cast('{{ column.name | upper }}' as {{ type_string() }}) as relation_column,
            cast('{{ column.dtype | upper }}' as {{ type_string() }}) as relation_column_type
        {% if not loop.last %}union all{% endif %}
        {% endfor %}
    ),

    test_data as (
        select *
        from relation_columns
        where relation_column = '{{ column_name }}'
        and relation_column_type not in ('{{ column_types | join("', '") }}')
    )

    select *
    from test_data
{% endmacro %}
