//! SAXPY on the GPU with cuda-oxide: `y = a * x + y`.
//!
//! SIMT style. The kernel body runs once per thread; each thread owns one
//! index. `DisjointSlice` hands each thread its own element of `y`, so no
//! two threads can write the same slot. rustc compiles the kernel to PTX
//! through a custom codegen backend and embeds it in the host binary.

use cuda_core::{CudaContext, DeviceBuffer, LaunchConfig1D};
use cuda_device::{DisjointSlice, kernel, launch_bounds, launch_contract, thread};
use cuda_host::cuda_module;

const N: usize = 1 << 20;
const A: f32 = 2.0;
/// Threads per block. The grid is N / BLOCK blocks, rounded up.
const BLOCK: u32 = 256;

#[cuda_module]
mod kernels {
    use super::*;

    /// One thread, one element. `get_mut` is `None` past the end of `y`.
    #[kernel]
    #[launch_bounds(256)]
    #[launch_contract(domain = 1, block = (256, 1, 1))]
    pub fn saxpy(a: f32, x: &[f32], mut y: DisjointSlice<f32>) {
        let idx = thread::index_1d();
        let i = idx.get();
        if let Some(y_i) = y.get_mut(idx) {
            *y_i = a * x[i] + *y_i;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = CudaContext::new(0)?;
    let stream = ctx.default_stream();

    let x_host: Vec<f32> = (0..N).map(|i| i as f32).collect();
    let y_host: Vec<f32> = vec![1.0; N];

    let x = DeviceBuffer::from_host(&stream, &x_host)?;
    let mut y = DeviceBuffer::from_host(&stream, &y_host)?;

    // SAFETY: this binary owns the embedded device bundle built from
    // the `kernels` module above.
    let module = unsafe { kernels::load(&ctx)? };
    let grid = (N as u32).div_ceil(BLOCK);
    let prepared = module.prepare_saxpy(LaunchConfig1D::new(grid, BLOCK, 0))?;
    module.saxpy(&stream, &prepared, A, &x, &mut y)?;

    let y_out = y.to_host_vec(&stream)?;

    println!("y[0..4] = {:?}", &y_out[0..4]);
    let errors = (0..N).filter(|&i| y_out[i] != A * x_host[i] + 1.0).count();
    if errors == 0 {
        println!("PASSED: all {N} elements correct");
    } else {
        eprintln!("FAILED: {errors} errors");
        std::process::exit(1);
    }
    Ok(())
}
