#!/usr/bin/env bash
# Fire the same pricing request at each department engine to show the
# header-based routing in action. Requires the API running on :8000.
set -euo pipefail

API="${API:-http://localhost:8000}"
BODY='{"sku":"WIDGET-001","base_price_cents":1000,"quantity":10,"segment":"vip"}'

echo "Request body: $BODY"
echo

for dept in retail wholesale unknown-dept ""; do
  label="${dept:-<no header>}"
  echo "=== X-Department: ${label} ==="
  if [ -n "$dept" ]; then
    curl -s -X POST "$API/price" \
      -H "Content-Type: application/json" \
      -H "X-Department: $dept" \
      -d "$BODY"
  else
    curl -s -X POST "$API/price" \
      -H "Content-Type: application/json" \
      -d "$BODY"
  fi
  echo
  echo
done
