"""Behavioural-quality eval runner for the ask_warehouse pilot.

This is *not* a unit test (those live in ``ask_warehouse/tests/``).
It speaks HTTP to a running Welcome instance and measures the LLM's
output quality against a hand-curated golden set.

Invoke via the CLI::

    uv run common ask validate
    uv run common ask validate --url http://localhost:8050
    uv run common ask validate --golden /path/to/custom.yaml

Pass criterion: ≥80% of golden-set cases land in their expected
outcome. Below that, expand ``glossary.yaml`` or ``few_shot.yaml``
before exposing the endpoint to real users — the audit table tells
you where the LLM is failing in practice; this runner tells you
*before* deployment.
"""

from __future__ import annotations

import datetime
from pathlib import Path
from typing import Any

import requests
import yaml

from common_py.ask_warehouse.mod import GOLDEN_SET_PATH


def check_case(
    case: dict[str, Any], response: dict[str, Any]
) -> tuple[bool, list[str]]:
    """Return (passed, failures) for one case against one /api/ask response."""
    failures: list[str] = []
    sql = response.get("sql")
    summary = response.get("summary", "")

    if case.get("must_be_unsure"):
        if sql is not None:
            failures.append(f"expected -- UNSURE but got SQL: {sql[:120]}…")
        return (not failures, failures)

    if sql is None:
        failures.append(f"expected SQL but got unsure / error: {summary}")
        return (False, failures)

    if case.get("must_execute") and "error" in summary.lower():
        failures.append(f"SQL did not execute cleanly: {summary}")

    sql_lower = sql.lower()
    for token in case.get("sql_must_contain", []):
        if token.lower() not in sql_lower:
            failures.append(f"SQL missing required token {token!r}")

    expected = case.get("expected_rows")
    actual = response.get("row_count")
    if expected and actual is not None:
        lo, hi = expected
        if not (lo <= actual <= hi):
            failures.append(f"row_count {actual} outside [{lo}, {hi}]")

    return (not failures, failures)


def run(
    *,
    url: str,
    golden_path: Path = GOLDEN_SET_PATH,
    email: str = "eval@northwind.local",
    pass_threshold: float = 0.8,
) -> int:
    """Run the eval against a live Welcome endpoint.

    Returns:
        Exit code: ``0`` if the pass rate meets ``pass_threshold``,
        else ``1``. The CLI wrapper turns this into a ``typer.Exit``.
    """
    with open(golden_path) as f:
        golden = yaml.safe_load(f)
    cases = golden["examples"]

    started = datetime.datetime.now(datetime.timezone.utc)
    print(f"# ask_warehouse eval — {len(cases)} cases against {url}")
    print(f"# started at {started.isoformat()}")
    print()

    passed = 0
    failed: list[tuple[str, list[str]]] = []

    for case in cases:
        case_id = case["id"]
        print(f"  • {case_id} …", end=" ", flush=True)
        try:
            r = requests.post(
                f"{url}/api/ask",
                json={"question": case["question"]},
                headers={"X-Auth-Email": email, "Content-Type": "application/json"},
                timeout=60,
            )
            r.raise_for_status()
            response = r.json()
        except Exception as e:  # noqa: BLE001 — surface HTTP errors as failures
            failed.append((case_id, [f"HTTP error: {e}"]))
            print("FAIL (HTTP)")
            continue

        ok, failures = check_case(case, response)
        if ok:
            passed += 1
            print(f"PASS  ({response.get('duration_ms')} ms)")
        else:
            failed.append((case_id, failures))
            print("FAIL")

    total = len(cases)
    rate = passed / total if total else 0.0
    print()
    print(f"# {passed}/{total} passed ({rate:.0%})")

    if failed:
        print()
        print("## Failures")
        for case_id, failures in failed:
            print(f"- {case_id}:")
            for f in failures:
                print(f"    {f}")

    return 0 if rate >= pass_threshold else 1
