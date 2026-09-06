# Dragonfly — WASM Plugin Example

A working example of the plugin architecture described in [genisis.md](genisis.md): a Rust plugin compiled to WebAssembly, loaded and called by a Python host via [Extism](https://extism.org).

The plugin implements two data pipeline operations:

- **validate** — checks business rules on a data record (region-specific product codes, positive quantities)
- **transform** — normalizes fields, computes totals, adds currency

## Prerequisites

```bash
# WASM compilation target for Rust
rustup target add wasm32-unknown-unknown

# Extism runtime (required by the Python SDK)
brew install extism/tap/extism

# Python dependencies
pip install extism
```

## Build the plugin

```bash
cd plugin
cargo build --release --target wasm32-unknown-unknown
```

This produces `plugin/target/wasm32-unknown-unknown/release/dragonfly_plugin.wasm`.

## Run the host

```bash
cd host
python run.py
```

## Expected output

```
=== Validation Pass ===

  [PASS] EU-1234
  [PASS] US-5678
  [FAIL] XX-BAD  -> quantity must be positive, product_code 'XX-BAD' does not match region 'US'
  [PASS] AP-9012

=== Transform Pass (3 valid records) ===

  EU-1234: 10 x 2500c = 25000c EUR (region: EU)
  US-5678: 5 x 1000c = 5000c USD (region: US)
  AP-9012: 20 x 750c = 15000c SGD (region: APAC)
```

## How it works

1. The Rust plugin compiles to a `.wasm` module with exported functions (`validate`, `transform`)
2. The Python host loads the `.wasm` via Extism's manifest system
3. Data crosses the WASM boundary as JSON — serialized by `serde` in Rust, `json` in Python
4. The WASM sandbox ensures the plugin cannot access the host's memory, filesystem, or network
5. Extism manages the memory allocation and function dispatch across the boundary

## Next steps

- Add host functions (let the plugin call back into the host for lookups)
- Explore the [Component Model](https://component-model.bytecodealliance.org/) and WIT files for typed interfaces
- Build a plugin registry for sharing and versioning plugins
- See [genisis.md](genisis.md) for the full architecture vision
