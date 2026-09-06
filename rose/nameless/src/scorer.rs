use crate::harness::HarnessResult;

const TIME_WEIGHT: f64 = 0.5;
const MEMORY_WEIGHT: f64 = 0.3;
const SIZE_WEIGHT: f64 = 0.1;
const COMPILE_WEIGHT: f64 = 0.1;

/// Compute a composite score normalized against the reference implementation.
///
/// Returns 0.0 if the candidate failed correctness tests.
/// Returns > 1.0 if the candidate outperforms the reference.
pub fn score(candidate: &HarnessResult, reference: &HarnessResult) -> f64 {
    if !candidate.passed {
        return 0.0;
    }

    let time = metric_ratio(reference.time_ns, candidate.time_ns);
    let memory = metric_ratio(reference.memory_bytes, candidate.memory_bytes);
    let size = metric_ratio(reference.binary_size, candidate.binary_size);
    let compile = metric_ratio(reference.compile_time_ns, candidate.compile_time_ns);

    TIME_WEIGHT * time + MEMORY_WEIGHT * memory + SIZE_WEIGHT * size + COMPILE_WEIGHT * compile
}

/// Ratio of reference/candidate, where higher = candidate is better.
/// Handles zero-value edge cases gracefully.
fn metric_ratio(reference: u64, candidate: u64) -> f64 {
    match (reference, candidate) {
        (0, 0) => 1.0,
        (0, _) => 0.5,
        (_, 0) => 2.0,
        (r, c) => r as f64 / c as f64,
    }
}
