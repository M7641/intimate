"""Turn an OpenAPI spec into a list of concrete GET URLs to hammer.

Instead of pointing gauge at one endpoint, point it at the app's `/openapi.json`
and it exercises the whole documented GET surface. This is deliberately modest:
GET only, path params filled with a sample value, required query params included
with an example. Endpoints needing a request body are skipped — driving those
realistically is a bigger problem than a load probe should solve.
"""

from __future__ import annotations

import json
import urllib.request


def fetch_spec(url: str) -> dict:
    with urllib.request.urlopen(url, timeout=10) as r:
        return json.load(r)


def _example_for(param: dict) -> str:
    """Best available sample value for a parameter, by decreasing specificity."""
    schema = param.get("schema", {})
    if "example" in param:
        return str(param["example"])
    for key in ("example", "default"):
        if key in schema:
            return str(schema[key])
    if schema.get("enum"):
        return str(schema["enum"][0])
    return {"integer": "1", "number": "1", "boolean": "true", "string": "example"}.get(
        schema.get("type", "string"), "example"
    )


def derive_get_urls(spec: dict, base_url: str) -> list[str]:
    """Concrete GET URLs from the spec's documented paths."""
    urls: list[str] = []
    for path, methods in spec.get("paths", {}).items():
        get = methods.get("get")
        if get is None:  # no GET operation — an empty-but-present one is still valid
            continue
        concrete = path
        query: list[str] = []
        for p in get.get("parameters", []):
            name, loc = p.get("name"), p.get("in")
            value = _example_for(p)
            if loc == "path":
                concrete = concrete.replace("{" + name + "}", value)
            elif loc == "query" and p.get("required"):
                query.append(f"{name}={value}")
        if "{" in concrete:  # an unfilled path param we couldn't sample
            continue
        url = base_url.rstrip("/") + concrete
        if query:
            url += "?" + "&".join(query)
        urls.append(url)
    return urls
