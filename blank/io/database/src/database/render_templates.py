from typing import Any
from jinja2 import Template


def render_query(sql_string: str, parameters: dict[str, Any] = {}) -> str:
    """
    renders a parameterised sql query using the given arguments
    """
    sql_string = sql_string.strip()
    tmpl = Template(sql_string)
    return tmpl.render(**parameters)
