{% test exists(model, column_name) -%}
    {{ return(adapter.dispatch('test_exists')(model, column_name)) }}
{% endtest %}

{% macro default__test_exists(model, column_name)  -%}
    select count({{ column_name }})
    from {{ model }}
    where 1=0
    limit 0
{% endmacro %}
