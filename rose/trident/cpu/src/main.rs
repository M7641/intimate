//! SAXPY on the CPU: `y = a * x + y`.
//!
//! The reference implementation. One loop, one thread, no device.

const N: usize = 1 << 20;
const A: f32 = 2.0;

/// Scale `x` by `a` and accumulate into `y`, element by element.
fn saxpy(a: f32, x: &[f32], y: &mut [f32]) {
    for (y_i, x_i) in y.iter_mut().zip(x) {
        *y_i = a * x_i + *y_i;
    }
}

fn main() {
    let x: Vec<f32> = (0..N).map(|i| i as f32).collect();
    let mut y: Vec<f32> = vec![1.0; N];

    saxpy(A, &x, &mut y);

    println!("y[0..4] = {:?}", &y[0..4]);
    let errors = (0..N).filter(|&i| y[i] != A * x[i] + 1.0).count();
    if errors == 0 {
        println!("PASSED: all {N} elements correct");
    } else {
        eprintln!("FAILED: {errors} errors");
        std::process::exit(1);
    }
}
