"""Pricing API — one host process, many department engines.

A single FastAPI app loads three WASM pricing plugins at startup and picks
one per request based on the `X-Department` header:

    X-Department: retail      -> retail engine
    X-Department: wholesale   -> wholesale engine
    (missing / unknown)       -> default engine

This is the architectural experiment the pilot is exploring: instead of
running two separate instances (one per department), run one instance and
switch the *logic* — not just config — per request, with each department's
rules isolated in its own sandboxed WASM module.
"""

import json
from contextlib import asynccontextmanager
from pathlib import Path

import extism
from fastapi import FastAPI, Header
from fastapi.responses import JSONResponse
from pydantic import BaseModel

# Workspace release output: plugins/target/wasm32-unknown-unknown/release/*.wasm
PLUGIN_DIR = (
    Path(__file__).parent.parent
    / "plugins"
    / "target"
    / "wasm32-unknown-unknown"
    / "release"
)

# Department name -> compiled WASM module. Cargo turns the `pricing-<x>`
# package name into a `pricing_<x>.wasm` artifact (hyphens become
# underscores). Adding a department = dropping a new .wasm here.
ENGINES = {
    "default": "pricing_default.wasm",
    "retail": "pricing_retail.wasm",
    "wholesale": "pricing_wholesale.wasm",
}

# Loaded plugin instances, keyed by department name. Populated at startup.
_plugins: dict[str, extism.Plugin] = {}


@asynccontextmanager
async def lifespan(_app: FastAPI):
    """Load every plugin once at startup and reuse the instances.

    Instantiating an Extism plugin compiles the WASM module — not something
    you want to pay per request. We hold the instances for the app's
    lifetime and clear them on shutdown.
    """
    for name, filename in ENGINES.items():
        path = PLUGIN_DIR / filename
        if not path.exists():
            raise RuntimeError(
                f"missing plugin '{name}' at {path} — run ./build.sh first"
            )
        # wasi=False: these plugins need no filesystem/network/clock.
        # The capability is simply never granted (least privilege).
        _plugins[name] = extism.Plugin({"wasm": [{"path": str(path)}]}, wasi=False)
    yield
    _plugins.clear()


app = FastAPI(title="Dragonfly Pricing API", lifespan=lifespan)


class PricingRequest(BaseModel):
    sku: str
    base_price_cents: int
    quantity: int
    segment: str | None = None


def _resolve_engine(requested: str | None) -> tuple[str, bool]:
    """Map a header value to a loaded engine.

    Returns (engine_name, fell_back). `fell_back` is True when the caller
    asked for an engine we don't have, so the response can be honest about
    the fact it was served by the default rather than what was requested.
    """
    if requested is None:
        return "default", False
    name = requested.strip().lower()
    if name in _plugins:
        return name, False
    return "default", True


@app.post("/price")
def price(req: PricingRequest, x_department: str | None = Header(default=None)):
    engine, fell_back = _resolve_engine(x_department)
    payload = req.model_dump_json()

    try:
        raw = _plugins[engine].call("price", payload)
        quote = json.loads(raw)
    except Exception as exc:  # noqa: BLE001 — crash isolation is the point
        # genisis.md: "If the module crashes, the pipeline logs the error
        # and continues with a default." A bad department plugin must not
        # take down pricing for the other department.
        raw = _plugins["default"].call("price", payload)
        quote = json.loads(raw)
        quote["engine"] = "default"
        quote["fallback_reason"] = f"{engine} engine errored: {exc}"

    # Surface the routing decision so the behaviour is observable.
    quote["routing"] = {
        "header": x_department,
        "engine_used": engine,
        "fell_back_to_default": fell_back,
    }
    return quote


@app.get("/engines")
def engines():
    """List the department engines this instance currently serves."""
    return {"available": sorted(_plugins.keys()), "default": "default"}


@app.get("/health")
def health():
    return JSONResponse({"status": "ok", "engines_loaded": len(_plugins)})


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8000)
