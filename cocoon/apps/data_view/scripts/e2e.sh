#!/usr/bin/env bash
# End-to-end harness: seeded Postgres (Podman/Docker) + the real data_view
# server (serving the built SPA) + Playwright against the live UI.
#
# One command, full teardown on exit:
#   apps/data_view/scripts/e2e.sh
#
# Env knobs: PG_PORT (default 55432), APP_PORT (default 8050),
#            SKIP_BUILD=1 to reuse an existing binary + dist.
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_DIR="$APP_DIR/frontend"
WS_ROOT="$(cd "$APP_DIR/../.." && pwd)"
BIN="$WS_ROOT/target/debug/data_view"

PG_PORT="${PG_PORT:-55432}"
APP_PORT="${APP_PORT:-8050}"
PG_NAME="dv-e2e-pg"

# Talk to Podman's Docker-compatible socket if Docker itself isn't up.
if ! docker info >/dev/null 2>&1; then
  if command -v podman >/dev/null 2>&1; then
    sock="$(podman machine inspect --format '{{.ConnectionInfo.PodmanSocket.Path}}' 2>/dev/null || true)"
    [ -S "$sock" ] && export DOCKER_HOST="unix://$sock"
  fi
fi
docker info >/dev/null 2>&1 || { echo "No container engine reachable (start Docker or 'podman machine start')"; exit 1; }

server_pid=""
cleanup() {
  if [ -n "$server_pid" ]; then
    # SIGTERM for a graceful drain, then SIGKILL so a slow shutdown can't leave
    # the port bound for the next run.
    kill "$server_pid" 2>/dev/null || true
    sleep 1
    kill -9 "$server_pid" 2>/dev/null || true
  fi
  docker rm -f "$PG_NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "==> Postgres ($PG_NAME) on :$PG_PORT"
docker rm -f "$PG_NAME" >/dev/null 2>&1 || true
docker run -d --name "$PG_NAME" -p "$PG_PORT:5432" \
  -e POSTGRES_PASSWORD=postgres docker.io/library/postgres:18-alpine >/dev/null
for _ in $(seq 1 30); do
  docker exec "$PG_NAME" pg_isready -U postgres >/dev/null 2>&1 && break
  sleep 1
done

echo "==> Seeding fixtures"
for f in "$APP_DIR"/tests/seeds/0*.sql; do
  docker exec -i "$PG_NAME" psql -U postgres -d postgres -q < "$f" >/dev/null
done

if [ "${SKIP_BUILD:-}" != "1" ]; then
  echo "==> Building server + frontend bundle"
  (cd "$WS_ROOT" && cargo build -p data_view --quiet)
  (cd "$FRONTEND_DIR" && bun run build >/dev/null)
fi

echo "==> Starting server on :$APP_PORT"
DATA_WAREHOUSE_TYPE=amazon_redshift \
REDSHIFT_HOST=127.0.0.1 REDSHIFT_PORT="$PG_PORT" REDSHIFT_DATABASE=postgres \
REDSHIFT_USERNAME=postgres REDSHIFT_PASSWORD=postgres REDSHIFT_SSL_MODE=disable \
DATA_VIEW_PORT="$APP_PORT" RUST_LOG=warn \
  "$BIN" serve >/tmp/dv_e2e_server.log 2>&1 &
server_pid="$!"
for _ in $(seq 1 30); do
  curl -fsS "http://127.0.0.1:$APP_PORT/health/ready" >/dev/null 2>&1 && break
  sleep 1
done

echo "==> Running Playwright"
(cd "$FRONTEND_DIR" && E2E_BASE_URL="http://127.0.0.1:$APP_PORT" bunx playwright test "$@")
