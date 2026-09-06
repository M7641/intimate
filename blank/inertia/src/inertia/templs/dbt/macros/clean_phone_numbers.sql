{% macro clean_phone_numbers(field) -%}
case
    when "{{ field }}" ilike '44/0%' and length(substring(regexp_replace("{{ field }}", '[a-zA-Z\\s.-]+', '') from 4)) <=7
        then null
    when length(regexp_replace("{{ field }}", '[a-zA-Z\\s.-]+', '')) <= 7
        then null
    when substring(regexp_replace("{{ field }}", '[a-zA-Z\\s.-]+', '') from 1 for 2) = '44'
        then '0' || substring(regexp_replace("{{ field }}", '[a-zA-Z\\s.-]+', '') from 3)
    else regexp_replace("{{ field }}", '[a-zA-Z\\s.-]+', '')
end
{%- endmacro %}
