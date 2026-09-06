"""
k6 load-test generator for FastAPI applications.

Extracts all documented routes from the app's OpenAPI spec, generates a k6
script that covers them automatically, then shells out to k6.

Usage (from any app CLI):
    run_load_test(fastapi_app, base_url="http://localhost:8050")
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from fastapi import FastAPI

# ---------------------------------------------------------------------------
# k6 script template
# ---------------------------------------------------------------------------

_TEMPLATE = """\
import http from 'k6/http';
import {{ check, sleep }} from 'k6';

export const options = {{
  vus: {vus},
  duration: '{duration}',
}};

const BASE_URL = __ENV.BASE_URL || '{base_url}';
const HEADERS = {{
  'X-Auth-Email':  __ENV.AUTH_EMAIL  || '{auth_email}',
  'X-Auth-Tenant': __ENV.AUTH_TENANT || '{auth_tenant}',
  'X-Auth-Role':   __ENV.AUTH_ROLE   || '{auth_role}',
  'Content-Type': 'application/json',
}};

export default function () {{
{body}
  sleep(1);
}}
"""

# ---------------------------------------------------------------------------
# Schema helpers
# ---------------------------------------------------------------------------


def _resolve_ref(ref: str, components: dict) -> dict:
    """Follow a JSON Pointer $ref inside `components`."""
    parts = ref.lstrip("#/").split("/")
    node: Any = {"components": components}
    for part in parts:
        node = node[part]
    return node


def _example_value(schema: dict, components: dict, _depth: int = 0) -> Any:
    """Return a minimal example value for a JSON Schema node."""
    if _depth > 8:
        return None
    if "$ref" in schema:
        schema = _resolve_ref(schema["$ref"], components)
    if "example" in schema:
        return schema["example"]
    if "examples" in schema:
        first = next(iter(schema["examples"].values()), {})
        if "value" in first:
            return first["value"]
    t = schema.get("type")
    if t == "object" or "properties" in schema:
        return {
            k: _example_value(v, components, _depth + 1)
            for k, v in schema.get("properties", {}).items()
        }
    if t == "array":
        return [_example_value(schema.get("items", {}), components, _depth + 1)]
    if t == "string":
        return schema.get("default", "string")
    if t == "integer":
        return schema.get("default", 0)
    if t == "number":
        return schema.get("default", 0.0)
    if t == "boolean":
        return schema.get("default", True)
    return None


def _fill_path_params(path: str, parameters: list[dict], components: dict) -> str:
    """Replace {name} placeholders with example values."""
    for param in parameters:
        if param.get("in") != "path":
            continue
        name = param["name"]
        example = param.get("example") or _example_value(
            param.get("schema", {}), components
        )
        path = path.replace(
            f"{{{name}}}", str(example) if example is not None else name
        )
    return path


# ---------------------------------------------------------------------------
# k6 scenario builder
# ---------------------------------------------------------------------------

_K6_METHOD = {
    "get": "http.get",
    "delete": "http.del",
    "head": "http.head",
    "options": "http.options",
}
_K6_BODY_METHOD = {"post", "put", "patch"}


def _scenario(path: str, method: str, operation: dict, components: dict) -> str:
    """Return a JS block for one OpenAPI operation."""
    params = operation.get("parameters", [])
    url = f"`${{BASE_URL}}{_fill_path_params(path, params, components)}`"
    expected = min(operation.get("responses", {200: None}), key=lambda s: int(s))
    label = f"{method.upper()} {path} → {expected}"

    if method in _K6_BODY_METHOD:
        body_schema = (
            operation.get("requestBody", {})
            .get("content", {})
            .get("application/json", {})
            .get("schema", {})
        )
        body_example = _example_value(body_schema, components) if body_schema else None
        body_js = (
            f"JSON.stringify({json.dumps(body_example)})"
            if body_example is not None
            else "null"
        )
        call = f"http.{method}({url}, {body_js}, {{ headers: HEADERS }})"
    else:
        fn = _K6_METHOD.get(method, f"http.{method}")
        call = f"{fn}({url}, {{ headers: HEADERS }})"

    return (
        f"  {{\n"
        f"    const res = {call};\n"
        f"    check(res, {{ {json.dumps(label)}: (r) => r.status === {expected} }});\n"
        f"  }}"
    )


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def generate_k6_script(
    app: FastAPI,
    *,
    base_url: str = "http://localhost:8050",
    vus: int = 10,
    duration: str = "30s",
    auth_email: str = "loadtest@localhost",
    auth_tenant: str = "dev",
    auth_role: str = "admin",
) -> str:
    """Return a k6 JS script covering every documented route in `app`."""
    spec = app.openapi()
    components = spec.get("components", {})
    scenarios: list[str] = []

    for path, methods in spec.get("paths", {}).items():
        for method, operation in methods.items():
            if method not in {
                "get",
                "post",
                "put",
                "patch",
                "delete",
                "head",
                "options",
            }:
                continue
            scenarios.append(_scenario(path, method, operation, components))

    return _TEMPLATE.format(
        vus=vus,
        duration=duration,
        base_url=base_url,
        auth_email=auth_email,
        auth_tenant=auth_tenant,
        auth_role=auth_role,
        body="\n".join(scenarios) if scenarios else "  // no documented routes found",
    )


def run_load_test(
    app: FastAPI,
    *,
    base_url: str = "http://localhost:8050",
    vus: int = 10,
    duration: str = "30s",
    auth_email: str = "loadtest@localhost",
    auth_tenant: str = "dev",
    auth_role: str = "admin",
) -> None:
    """Generate a k6 script from `app`'s OpenAPI spec and run it."""
    script = generate_k6_script(
        app,
        base_url=base_url,
        vus=vus,
        duration=duration,
        auth_email=auth_email,
        auth_tenant=auth_tenant,
        auth_role=auth_role,
    )
    with tempfile.NamedTemporaryFile(
        suffix=".js", mode="w", delete=False, prefix="k6_"
    ) as f:
        f.write(script)
        script_path = Path(f.name)

    try:
        result = subprocess.run(["k6", "run", str(script_path)], check=False)
        sys.exit(result.returncode)
    finally:
        script_path.unlink(missing_ok=True)
