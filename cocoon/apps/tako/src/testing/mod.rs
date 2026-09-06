//! Test and benchmark tooling for the upload API.
//!
//! None of this runs on the production `serve` path: it backs only the
//! `gen-data`, `test-upload`, `benchmark` and `bench-parse` subcommands. Kept in
//! its own module so the server logic and the throwaway tooling stay separate.
pub mod bench;
pub mod testkit;

// End-to-end HTTP test against an in-process DuckDB. Test builds only — it is a
// `#[test]`, never part of the binary even with `--features testing`.
#[cfg(test)]
mod integration_tests;
