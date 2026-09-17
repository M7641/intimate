//! SAXPY on the GPU with cuTile Rust: `y = a * x + y`.
//!
//! The kernel body runs once per tile, not once per thread. The compiler
//! maps the tile onto threads and registers. `y` is partitioned into
//! disjoint tiles before launch, so each tile has exclusive write access.
//! The Rust AST is JIT-compiled through CUDA Tile IR on first launch.

use cutile::prelude::*;

const N: usize = 1 << 20;
const A: f32 = 2.0;
/// Elements per tile. The grid is inferred: N / TILE tiles.
const TILE: usize = 1024;

#[cutile::module]
mod kernel {
    use cutile::core::*;

    /// `y` is one tile of the output. `x` is the whole input; `load_like`
    /// fetches the slice of `x` that lines up with this tile of `y`.
    #[cutile::entry()]
    fn saxpy<const B: i32>(y: &mut Tensor<f32, { [B] }>, a: f32, x: &Tensor<f32, { [-1] }>) {
        let tile_a = a.broadcast(y.shape());
        let tile_x = x.load_like(y);
        let tile_y = y.load();
        y.store(tile_a * tile_x + tile_y);
    }
}

fn main() -> Result<(), Error> {
    let device = Device::new(0)?;
    let stream = device.new_stream()?;

    let x_host: Arc<Vec<f32>> = Arc::new((0..N).map(|i| i as f32).collect());
    let y_host: Arc<Vec<f32>> = Arc::new(vec![1.0; N]);

    // Upload. Shared input becomes an Arc; the output is partitioned.
    let x: Arc<Tensor<f32>> = api::copy_host_vec_to_device(&x_host)
        .sync_on(&stream)?
        .into();
    let y = api::copy_host_vec_to_device(&y_host)
        .sync_on(&stream)?
        .partition([TILE]);

    // Launch returns the arguments in parameter order; `y` comes first.
    let y_out: Vec<f32> = kernel::saxpy(y, A, x)
        .first()
        .unpartition()
        .to_host_vec()
        .sync_on(&stream)?;

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
