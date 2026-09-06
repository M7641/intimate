{% macro convert_to_date(field) -%}
case
    when length("{{ field }}") <= 7 then null
    when "{{ field }}" similar to '[0-9]{4}[0-9]{2}[0-9]{2}'
        then to_date("{{ field }}" , 'YYYYMMDD')
    when "{{ field }}" similar to '[0-9]{4}-[0-9]{2}-[0-9]{2}'
        then "{{ field }}"::date
    when "{{ field }}" similar to '[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}'
        then "{{ field }}"::date
    when "{{ field }}" similar to '[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}.[0-9]+'
        then "{{ field }}"::date
    when "{{ field }}"  similar to '[0-9]{2}/[0-9]{2}/[0-9]{4}'
        then to_date("{{ field }}" , 'DD/MM/YYYY')
    when lower("{{ field }}") similar to '[0-9]{2}-[a-z]{3}-[0-9]{4}'
        then to_date(lower("{{ field }}"), 'DD-mon-YYYY')
    when lower("{{ field }}") similar to '[0-9]{2}-[a-z]{3}-[0-9]{2}'
        then to_date(lower("{{ field }}"), 'DD-mon-YY')
    when lower(replace("{{ field }}", ' ', '-')) similar to '[a-z]{4,9}-[0-9]{4}'
        then to_date(lower("{{ field }}"), 'Month-YYYY')
    when lower(replace("{{ field }}", ' ', '-')) similar to '(may){1}-[0-9]{4}' -- Only three letter month.
        then to_date(lower("{{ field }}"), 'Month-YYYY')
    when lower(replace("{{ field }}", ' ', '-')) similar to '[a-z]{3}-[0-9]{2}'
        then to_date(lower("{{ field }}"), 'mon-YY')
    when lower(replace("{{ field }}", ' ', '-')) similar to '[a-z]{3}-[0-9]{4}'
        then to_date(lower("{{ field }}"), 'mon-YYYY')
    when lower(replace("{{ field }}", ' ', '-')) similar to '[a-z]{3}/[0-9]{2}'
        then to_date(lower("{{ field }}"), 'mon/YY')
    when lower(replace("{{ field }}", ' ', '-')) similar to '[0-9]{4}[0-9]{2}'
        then to_date(lower("{{ field }}"), 'YYYYMM')
    when lower(replace("{{ field }}", '_', '')) similar to '[0-9]{4}(q1){1}'
        then to_date(substring("{{ field }}" from 1 for 4) || '-01-01', 'YYYY-MM-DD')
    when lower(replace("{{ field }}", '_', '')) similar to '[0-9]{4}(q2){1}'
        then to_date(substring("{{ field }}" from 1 for 4) || '-04-01', 'YYYY-MM-DD')
    when lower(replace("{{ field }}", '_', '')) similar to '[0-9]{4}(q3){1}'
        then to_date(substring("{{ field }}" from 1 for 4) || '-07-01', 'YYYY-MM-DD')
    when lower(replace("{{ field }}", '_', '')) similar to '[0-9]{4}(q4){1}'
        then to_date(substring("{{ field }}" from 1 for 4) || '-10-01', 'YYYY-MM-DD')
    when "{{ field }}" similar to '[0-9]{1,2}/[0-9]{1,2}/[0-9]{4} [0-9]{1,2}:[0-9]{2}:[0-9]{2} (AM|PM)'
        then to_date("{{ field }}", 'MM/DD/YYYY HH12:MI:SS AM')
    else null
end
{%- endmacro %}
