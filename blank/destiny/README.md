# Destiny

The purpose of this module is to reaplce situations where RouteVendor was used when there was no actual vehicle routing happening in the solution. In particular, BrewCo and Orchard Foods where it was just used to get distances between two points.

Therefore, we have taken OSRM and wrapped it in a simple to use class that can be used to get routes and distances between points.

I did play around with [VRP](https://github.com/nimbus-labs/ds-routing-vrp/tree/master), the rust based routing engine, but without any use cases, there was no need in taking it further.

At any rate, I'll leave in the code from my limited experiments with VRP as it includes a way to create the right input objets and how to install VRP should we need it in the future.

## OSRM

The OSRM instance we have here just has the maps for Britain and Ireland loaded. It's not hard to add new ones, but I don't understand what the cost of massive Docker images are which is the main limitation.

## Postcodes

The other aspect to mention is I did inlucde some utils to map postcodes to lat/lon using https://api.postcodes.io.

## Project layout — tests next to the code (Rust-style)

Each feature is a **folder** under `src/destiny/` holding its source, its tests
and its benchmark side by side:

```
src/destiny/
├── __init__.py            # public API, unchanged: `from destiny import Destiny`
├── cli.py
├── destiny/               # feature
│   ├── __init__.py        # re-exports → `from destiny.destiny import Destiny`
│   ├── destiny.py         # source
│   ├── destiny_test.py    # unit tests, next to the code
│   └── destiny_bench.py   # benchmark (hot path)
├── postmacode/  (postmacode.py, postmacode_test.py)
└── overpass/    (overpass.py, overpass_test.py, overpass_bench.py)
```

Each `__init__.py` re-exports the feature's public symbols, so the package
behaves exactly as before. pytest collects `test_*.py` / `*_test.py`
(`python_files` in `pyproject.toml`); `*_bench.py` files are **excluded** from
the normal test run and only picked up by the `bench` task.

```bash
moon run destiny:test    # unit tests only (fast)
```

## Performance gate — a budget, not a hope

The hot paths (`extract_route_nodes`, `compute_haversine_distance`,
`create_travel_matrix` — the O(n²) one) are measured with
[`pytest-benchmark`](https://pytest-benchmark.readthedocs.io). A committed
baseline (`.benchmarks/baseline.json`) is the budget; CI fails the build if any
benchmark regresses beyond the threshold.

```bash
moon run destiny:bench           # measure + gate against the baseline
moon run destiny:bench-update    # regenerate the baseline (run on a stable host)
```

- **Gate logic** lives in `bench_gate.py`: it compares the current run to the
  baseline **by benchmark name** (machine-agnostic), on the least-noisy stat
  (`min`). It is preferred over pytest-benchmark's built-in
  `--benchmark-compare-fail` because that tool keys its storage by machine tag,
  so a committed baseline silently fails to match on a different CI runner.
- **Threshold**: `BENCH_MAX_REGRESSION` (percent, default `25`).
  `BENCH_STAT` selects the compared statistic (default `min`).
- **CI**: `moon ci` runs `destiny:bench` (it's `runInCI: yes`), so a regression
  beyond the threshold blocks the merge — make `ci` a required check.

> The baseline is machine-sensitive. For the gate to be meaningful in CI, the
> baseline should be regenerated on (or matched to) the CI runner; the threshold
> absorbs normal cross-run noise. This is a prototype of the pattern we want on
> the hot path of every service (moss, tako, warehouse, …).
