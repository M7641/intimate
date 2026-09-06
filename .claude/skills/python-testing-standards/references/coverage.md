# Coverage (complementary)

Coverage is not one of the five test kinds — it measures how much of the code the
tests touch, not whether the code is correct or fast. Use it to *find untested
code*, then write the missing unit/property tests. Don't gate hard on a coverage
percentage: it's easy to game (lines executed ≠ behaviour asserted) and chasing a
number produces assertion-free tests. **Mutation testing is the stronger signal**
for "are the tests real" — prefer it where you'd be tempted to set a coverage gate.

## pytest-cov / coverage.py

Dev-dep `pytest-cov`. Run alongside the normal suite:

```bash
uv run --no-sync pytest --cov=src/<pkg> --cov-report=term-missing
```

`--cov-report=xml` for CI ingestion. If you must gate, ratchet (`--cov-fail-under`
set to the current floor and only raised deliberately), never a fixed aspirational
number. Pair with `mutmut` for the real "are these tests doing anything" answer.
