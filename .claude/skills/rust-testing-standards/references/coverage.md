# Coverage (complementary)

Coverage is not one of the five test kinds — it measures how much of the code the
tests touch, not whether the code is correct or fast. Use it to *find untested
code*, then write the missing unit/property tests. Don't gate hard on a coverage
percentage: it's easy to game (lines executed ≠ behaviour asserted) and chasing a
number produces assertion-free tests. **Mutation testing is the stronger signal**
for "are the tests real" — prefer `cargo-mutants` where you'd be tempted to set a
coverage gate.

## cargo-llvm-cov

CLI tool → proto vendored plugin (`proto-plugins/cargo-llvm-cov.toml`), like
`cargo-deny`.

```bash
cargo llvm-cov --workspace --lcov --output-path lcov.info
cargo llvm-cov --summary-only          # quick human-readable view
```

Same rule: use it to locate gaps, ratchet if you gate at all. Pair with
`cargo-mutants` for the real "are these tests doing anything" answer.
