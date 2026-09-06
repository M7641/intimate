# unity — Rust → WebAssembly checkerboard

A small pilot that compiles a Rust function to WebAssembly and renders the
result to a `<canvas>` in the browser. A FastAPI server serves the static
front-end.

## Layout

| Path               | Purpose                                                         |
| ------------------ | --------------------------------------------------------------- |
| `two/`             | Rust crate (`cdylib`) compiled to WASM                          |
| `two/src/lib.rs`   | `color_checkerboard()` — returns an RGBA pixel buffer           |
| `public/`          | static front-end, served as-is                                  |
| `public/script.js` | loads the WASM module and draws the checkerboard                |
| `public/wasm/`     | generated glue + `.wasm` (not versioned — see `mise run build`) |
| `api.py`           | FastAPI static file server                                      |

## Toolchain

This project uses the post-`wasm-pack` workflow: a plain `cargo build` for the
`wasm32-unknown-unknown` target, then `wasm-bindgen` to emit the JS glue and
`wasm-opt` to shrink the binary. [mise](https://mise.jdx.dev) pins and installs
everything (Rust + the `wasm32` target, `wasm-bindgen`, `binaryen`).

```sh
curl -sSf https://mise.dev/install.sh | sh   # if you don't already have mise
cd two
mise trust && mise install
mise run build                                # -> ../public/wasm/
```

> The `wasm-bindgen` CLI version in `mise.toml` must match the `wasm-bindgen`
> crate version in `Cargo.lock`, or the build fails with a schema-mismatch
> error.

## Run

```sh
uv run api.py        # serves http://127.0.0.1:8000
```

(or `python api.py` in an environment with the `pyproject.toml` deps installed.)

The server registers the `application/wasm` MIME type so the browser can use the
fast `WebAssembly.instantiateStreaming` path.
