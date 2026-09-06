#!/usr/bin/env bash
# Build all department pricing plugins to WASM.
set -euo pipefail

cd "$(dirname "$0")/plugins"

# Ensure the WASM target is available.
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true

echo "Building department plugins -> WASM ..."
cargo build --release --target wasm32-unknown-unknown

echo
echo "Built modules:"
ls -1 target/wasm32-unknown-unknown/release/*.wasm
