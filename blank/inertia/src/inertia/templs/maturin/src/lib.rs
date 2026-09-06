use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
mod maturin {
    use pyo3::prelude::*;

    /// Formats the sum of two numbers as string.
    #[pyfunction]
    fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
        Ok((a + b).to_string())
    }

    /// Sums all elements in a vector.
    #[pyfunction]
    fn sum_vector(numbers: Vec<f64>) -> PyResult<f64> {
        Ok(numbers.iter().sum())
    }
}
