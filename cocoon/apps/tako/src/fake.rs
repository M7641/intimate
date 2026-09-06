//! Synthetic sample frames for tests and the `testing` CLI.
//!
//! The generic, reusable generator lives in the [`faker`] crate; this module is
//! just the tako-specific bridge from it to a Polars `DataFrame`. Compiled only
//! for tests and the `testing` feature — never in a production build.

use faker::Faker;
use polars::prelude::*;

/// The canonical sample frame used by the parse round-trip tests and the
/// `gen-data` / benchmark CLI: `id, name, age, score, active` over `rows` rows.
///
/// Seeded via [`faker`], so the contents are identical across runs.
pub fn sample_frame(rows: usize) -> PolarsResult<DataFrame> {
    let mut f = Faker::new(0x5EED_1234_5678_9ABC);

    let ids: Vec<i64> = (1..=rows as i64).collect();
    let names: Vec<String> = (0..rows).map(|_| f.name()).collect();
    let ages: Vec<i64> = (0..rows).map(|_| f.int_in(18, 80)).collect();
    let scores: Vec<f64> = (0..rows).map(|_| f.float_to(100.0)).collect();
    let active: Vec<bool> = (0..rows).map(|_| f.boolean()).collect();

    df! {
        "id" => &ids,
        "name" => &names,
        "age" => &ages,
        "score" => &scores,
        "active" => &active,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_frame_has_requested_shape() {
        let df = sample_frame(100).unwrap();
        assert_eq!(df.height(), 100);
        assert_eq!(df.width(), 5);
    }

    #[test]
    fn same_seed_is_reproducible() {
        let a = sample_frame(20).unwrap();
        let b = sample_frame(20).unwrap();
        assert!(a.equals(&b), "the seeded generator must be deterministic");
    }
}
