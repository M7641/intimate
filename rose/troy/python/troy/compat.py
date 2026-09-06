"""JSON Schema backward-compatibility checker for the marts surface.

oasdiff covers OpenAPI; `buf breaking` covers Protobuf. Raw JSON Schema (the dbt
mart contracts) has no equivalent off-the-shelf tool, so this is the custom
piece of the versioning layer.

CONTRACT DIRECTION. A mart schema describes what the producer (dbt) GUARANTEES
to consumers. A change is BREAKING when it weakens a guarantee a consumer relied
on, or narrows the set of values previously promised:

  BREAKING                                   SAFE (additive)
  --------                                   ---------------
  remove a property                          add an optional property
  add a newly-required property              remove a property from `required`*
  narrow a type union  (["number","null"]    widen a type union
                        -> ["number"])
  remove an enum value                       add an enum value
  raise `minimum` / lower `maximum`          lower `minimum` / raise `maximum`
  close additionalProperties (true -> false) open additionalProperties

  * still flagged WARN — a no-longer-guaranteed field can surprise consumers.

`pattern` changes are flagged WARN: sub/superset is undecidable statically.

Usage:
    python -m troy.compat BASE.json REVISION.json [--label NAME] [--at a.b.c]

Exits non-zero if any BREAKING finding is reported.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import yaml

BREAKING = "BREAKING"
WARN = "WARN"
ADDITIVE = "additive"


def _load(path: Path, at: str | None) -> dict:
    raw = path.read_text()
    doc = yaml.safe_load(raw) if path.suffix in {".yaml", ".yml"} else json.loads(raw)
    if at:
        for key in at.split("."):
            doc = doc[key]
    return doc


def _types(schema: dict) -> set[str] | None:
    t = schema.get("type")
    if t is None:
        return None
    return {t} if isinstance(t, str) else set(t)


def _compare(base: dict, rev: dict, path: str, out: list[tuple[str, str, str]]) -> None:
    """Recursively compare two object schemas, appending (level, path, message)."""
    here = path or "<root>"

    # ── type ──
    bt, rt = _types(base), _types(rev)
    if bt is not None and rt is not None and rt < bt:
        out.append((BREAKING, here, f"type narrowed {sorted(bt)} -> {sorted(rt)}"))
    elif bt is None and rt is not None:
        out.append((BREAKING, here, f"type tightened from any -> {sorted(rt)}"))
    elif bt is not None and rt is not None and bt < rt:
        out.append((ADDITIVE, here, f"type widened {sorted(bt)} -> {sorted(rt)}"))

    # ── enum ──
    be, re_ = base.get("enum"), rev.get("enum")
    if be is not None:
        removed = [v for v in be if re_ is None or v not in re_]
        if removed:
            out.append((BREAKING, here, f"enum values removed: {removed}"))
        if re_ is not None:
            added = [v for v in re_ if v not in be]
            if added:
                out.append((ADDITIVE, here, f"enum values added: {added}"))
    elif re_ is not None:
        out.append((BREAKING, here, "enum constraint added where none existed"))

    # ── numeric bounds ──
    if "minimum" in base and rev.get("minimum", base["minimum"]) > base["minimum"]:
        out.append(
            (BREAKING, here, f"minimum raised {base['minimum']} -> {rev['minimum']}")
        )
    if "minimum" not in base and "minimum" in rev:
        out.append((BREAKING, here, f"minimum constraint added (>= {rev['minimum']})"))
    if "maximum" in base and rev.get("maximum", base["maximum"]) < base["maximum"]:
        out.append(
            (BREAKING, here, f"maximum lowered {base['maximum']} -> {rev['maximum']}")
        )
    if "maximum" not in base and "maximum" in rev:
        out.append((BREAKING, here, f"maximum constraint added (<= {rev['maximum']})"))

    # ── pattern (undecidable) ──
    if base.get("pattern") != rev.get("pattern") and (
        base.get("pattern") or rev.get("pattern")
    ):
        out.append(
            (
                WARN,
                here,
                f"pattern changed {base.get('pattern')!r} -> {rev.get('pattern')!r}",
            )
        )

    # ── additionalProperties ──
    if (
        base.get("additionalProperties", True)
        and rev.get("additionalProperties", True) is False
    ):
        out.append((BREAKING, here, "additionalProperties closed (true -> false)"))

    # ── required ──
    breq, rreq = set(base.get("required", [])), set(rev.get("required", []))
    for field in sorted(rreq - breq):
        out.append((BREAKING, f"{here}.{field}", "field became required"))
    for field in sorted(breq - rreq):
        out.append(
            (WARN, f"{here}.{field}", "field no longer required (guarantee weakened)")
        )

    # ── properties (structure + recurse) ──
    bprops, rprops = base.get("properties", {}), rev.get("properties", {})
    for field in sorted(set(bprops) - set(rprops)):
        out.append((BREAKING, f"{here}.{field}", "property removed"))
    for field in sorted(set(rprops) - set(bprops)):
        out.append((ADDITIVE, f"{here}.{field}", "property added"))
    for field in sorted(set(bprops) & set(rprops)):
        _compare(bprops[field], rprops[field], f"{here}.{field}", out)


def check(base: dict, rev: dict) -> list[tuple[str, str, str]]:
    findings: list[tuple[str, str, str]] = []
    _compare(base, rev, "", findings)
    return findings


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="JSON Schema backward-compat checker")
    ap.add_argument("base", type=Path, help="baseline schema (the version on main)")
    ap.add_argument("revision", type=Path, help="proposed schema")
    ap.add_argument("--label", default="schema", help="name shown in output")
    ap.add_argument(
        "--at", help="dotted path to a sub-schema (e.g. components.schemas.X)"
    )
    args = ap.parse_args(argv)

    findings = check(_load(args.base, args.at), _load(args.revision, args.at))
    breaking = [f for f in findings if f[0] == BREAKING]

    colour = {BREAKING: "\033[31m", WARN: "\033[33m", ADDITIVE: "\033[32m"}
    print(f"\n  {args.label}: {len(findings)} change(s)")
    for level, where, msg in findings:
        print(f"    {colour[level]}{level:9}\033[0m {where}: {msg}")

    if breaking:
        print(f"  \033[31m❌ {len(breaking)} breaking change(s)\033[0m")
        return 1
    print("  \033[32m✅ backward-compatible\033[0m")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
