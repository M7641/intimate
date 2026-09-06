{% macro add_primary_key(target_table, target_column) %}
    {{ return(adapter.dispatch('add_primary_key')(target_table, target_column)) }}
{% endmacro %}

{% macro redshift__add_primary_key(target_table, target_column) %}
    {% set sql %}
    alter table {{ target_table }} add column temp_col varchar(100) not null default '';
    alter table {{ target_table }} alter distkey temp_col;
    update {{ target_table }} set temp_col = {{ target_column }};
    alter table {{ target_table }} drop column {{ target_column }};
    alter table {{ target_table }} rename column temp_col to {{ target_column }};
    alter table {{ target_table }} add primary key ({{ target_column }});
    {% endset %}

    {% do run_query(sql) %}
{% endmacro %}

{% macro snowflake__add_primary_key(target_table, target_column) %}
    {% set sql %}
    alter table {{ target_table }} add primary key ({{ target_column }});
    {% endset %}

    {% do run_query(sql) %}
{% endmacro %}
