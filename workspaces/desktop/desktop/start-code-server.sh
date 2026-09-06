#!/usr/bin/env bash
#
# start-code-server — launch code-server (VS Code) from inside the desktop, open in Firefox.
#
# Bound to localhost with no auth: only reachable within this pod, behind the desktop's
# (and Nimbus's) auth. Settings/extensions persist in ~/.local/share/code-server. Reuses an
# already-running instance.
#
set -euo pipefail
PORT="${CODE_SERVER_PORT:-8080}"
URL="http://127.0.0.1:${PORT}/"

if ! curl -s -o /dev/null -m 2 "$URL"; then
  echo "Starting code-server on 127.0.0.1:${PORT} …"
  nohup code-server --bind-addr "127.0.0.1:${PORT}" --auth none --disable-telemetry \
        >"$HOME/.code-server.log" 2>&1 &
  for _ in $(seq 1 30); do curl -s -o /dev/null -m 2 "$URL" && break; sleep 1; done
fi

exec firefox "$URL"
