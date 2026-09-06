{% macro convert_to_float(field) -%}
case
    when regexp_replace("{{ field }}"::varchar, '[^0-9.]', '') = '' then null
    when regexp_replace("{{ field }}"::varchar, '[^0-9]', '') = '' then null
    when "{{ field }}" like '%-%' then -cast(regexp_replace("{{ field }}"::varchar, '[^0-9.]', '') as float)
    else abs(cast(regexp_replace("{{ field }}"::varchar, '[^0-9.]', '') as float))
end
{%- endmacro %}
