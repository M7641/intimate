#!/usr/bin/env bash
# Build the engine for the browser and drop it where Vite serves it.
# The wasm32 target comes from ../rust-toolchain.toml; rustup adds it on first use.
set -euo pipefail
cd "$(dirname "$0")/.."
# rustup's cargo (no proto shim exists for Rust) ahead of any distro cargo.
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -p atlantis-wasm --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/atlantis_wasm.wasm web/public/atlantis.wasm
ls -l web/public/atlantis.wasm
