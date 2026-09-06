#!/usr/bin/env bash
# check-compat.sh — backward-compatibility gate for the troy contract registry.
#
# Runs all three surface checks against a git baseline (default: origin/main):
#   * proto    -> buf breaking   (skipped if buf is not installed)
#   * openapi  -> oasdiff        (skipped if oasdiff is not installed)
#   * marts    -> troy.compat    (always runs; needs uv)
#
# Same script is called by `make check-compat` and by CI, so local == CI.
# Usage: scripts/check-compat.sh [BASE_REF]
#
# Not `set -e`: we want every surface checked and the failures aggregated.
set -uo pipefail

BASE_REF="${1:-origin/main}"
ROOT="$(git rev-parse --show-toplevel)"
TROY="$ROOT/rose/troy"
BASE_SHA="$(git -C "$ROOT" rev-parse "$BASE_REF" 2>/dev/null || true)"
fail=0

section() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }

# --- 1. Protobuf -----------------------------------------------------------
section "buf breaking (proto)"
if ! command -v buf >/dev/null 2>&1; then
  echo "  SKIP — buf not installed (brew install bufbuild/buf/buf)"
elif [ -z "$BASE_SHA" ]; then
  echo "  SKIP — baseline '$BASE_REF' not found"
else
  if ( cd "$TROY/registry/proto" \
       && buf lint \
       && buf breaking --against "$ROOT/.git#ref=$BASE_SHA,subdir=rose/troy/registry/proto" ); then
    echo "  ✅ no breaking proto changes"
  else
    echo "  ❌ proto breaking changes"; fail=1
  fi
fi

# --- 2. OpenAPI ------------------------------------------------------------
section "oasdiff breaking (openapi)"
SPEC="rose/troy/registry/openapi/ingest.yaml"
if ! command -v oasdiff >/dev/null 2>&1; then
  echo "  SKIP — oasdiff not installed (go install github.com/oasdiff/oasdiff@latest)"
elif [ -z "$BASE_SHA" ]; then
  echo "  SKIP — baseline '$BASE_REF' not found"
else
  base_tmp="$(mktemp).yaml"
  if git -C "$ROOT" show "$BASE_SHA:$SPEC" > "$base_tmp" 2>/dev/null; then
    if oasdiff breaking "$base_tmp" "$ROOT/$SPEC" --fail-on ERR; then
      echo "  ✅ no breaking openapi changes"
    else
      echo "  ❌ openapi breaking changes"; fail=1
    fi
  else
    echo "  SKIP — no baseline for $SPEC (new file)"
  fi
  rm -f "$base_tmp"
fi

# --- 3. Marts (JSON Schema) ------------------------------------------------
section "json-schema compat (marts)"
SCHEMA="rose/troy/registry/marts/dim_customer.schema.json"
if [ -z "$BASE_SHA" ]; then
  echo "  SKIP — baseline '$BASE_REF' not found"
else
  base_tmp="$(mktemp).json"
  if git -C "$ROOT" show "$BASE_SHA:$SCHEMA" > "$base_tmp" 2>/dev/null; then
    if ( cd "$TROY/python" \
         && uv run --quiet python -m troy.compat "$base_tmp" "$ROOT/$SCHEMA" --label dim_customer ); then
      :
    else
      fail=1
    fi
  else
    echo "  SKIP — no baseline for $SCHEMA (new file)"
  fi
  rm -f "$base_tmp"
fi

# --- result ----------------------------------------------------------------
section "result"
if [ "$fail" -ne 0 ]; then
  echo "  ❌ breaking changes detected — bump the version or revert"
  exit 1
fi
echo "  ✅ all surfaces backward-compatible"
