{% macro clean_post_codes(field) -%}
case
    when length(trim("{{ field }}")) = 0
    then null
    else trim("{{ field }}")
end
{%- endmacro %}
