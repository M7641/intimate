# trident

One idea, three ways: SAXPY (`y = a * x + y`) over a million floats.

| crate   | where | model | toolchain |
|---------|-------|-------|-----------|
| `cpu`   | CPU   | one loop | stable Rust |
| `tile`  | GPU   | cuTile Rust: one kernel call per tile | stable Rust, JIT via CUDA Tile IR |
| `oxide` | GPU   | cuda-oxide: one kernel call per thread (SIMT) | pinned nightly, custom rustc backend |

Each binary prints the first four results and `PASSED` or `FAILED`.

## What differs

`cpu/src/main.rs`: a loop over slices. The reference.

`tile/src/main.rs`: the kernel receives one tile of `y` and the whole of
`x`. `load_like` picks the matching slice of `x`. No thread index, no
bounds check. The host partitions `y` into tiles; the grid follows.

`oxide/src/main.rs`: the kernel receives all of `y` as a `DisjointSlice`
and reads its thread index. `get_mut` returns `None` past the end. The
host picks block and grid size and proves them against a launch contract.

## Requirements

- NVIDIA GPU, compute capability 8.0+ (tested: RTX 3080, sm_86).
- CUDA toolkit 13.2+ at `/opt/cuda` (path set in `.cargo/config.toml`).
- Driver R580+.
- clang with libclang, for bindgen in the host bindings.

## Run

cpu and tile share the workspace:

```bash
cargo run --release -p trident-cpu
cargo run --release -p trident-tile
```

oxide is a standalone crate. It needs `cargo-oxide` built from the same
commit its dependencies pin:

```bash
rustup toolchain install nightly-2026-08-28 \
  --component rust-src,rustc-dev,llvm-tools,rustfmt,clippy
cargo +nightly-2026-08-28 install --git https://github.com/NVlabs/cuda-oxide.git \
  --rev 7ce30ec798c490e6ed772dfcbad193b3733fe55a cargo-oxide

export CUDA_TOOLKIT_PATH=/opt/cuda   # doctor probes /usr/local/cuda otherwise
cd oxide
cargo oxide doctor
cargo oxide run                      # builds release by default
```

## Sources

- cutile-rs: https://github.com/NVlabs/cutile-rs
- cuda-oxide: https://github.com/NVlabs/cuda-oxide
- NVIDIA blog, "Introducing CUDA Rust":
  https://developer.nvidia.com/blog/introducing-cuda-rust-two-tracks-for-writing-gpu-kernels/
