"""Static file server for the Rust + WebAssembly checkerboard demo.

Serves the ``public/`` directory (HTML, JS and the generated WASM glue).
Run with ``uv run api.py`` after building the WASM (``mise run build``).
"""

import mimetypes
from pathlib import Path

from fastapi import FastAPI
from fastapi.staticfiles import StaticFiles

# WebAssembly.instantiateStreaming() only accepts a response served as
# `application/wasm`; register it explicitly so we don't depend on the host's
# mimetypes database having the right entry.
mimetypes.add_type("application/wasm", ".wasm")

PUBLIC_DIR = Path(__file__).parent / "public"

app = FastAPI(title="Rust + WASM checkerboard")
app.mount("/", StaticFiles(directory=PUBLIC_DIR, html=True), name="public")

if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="127.0.0.1", port=8000, log_level="info")
