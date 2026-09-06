#!/usr/bin/env bash
#
# start-jupyterlab — launch JupyterLab from inside the desktop and open it in Firefox.
#
# Bound to localhost with no token: it's only reachable from within this pod, behind the
# desktop's (and Nimbus's) auth. Reuses an already-running instance if there is one.
#
set -euo pipefail
PORT="${JUPYTERLAB_PORT:-8888}"
URL="http://127.0.0.1:${PORT}/lab"

if ! curl -s -o /dev/null -m 2 "$URL"; then
  echo "Starting JupyterLab on 127.0.0.1:${PORT} …"
  nohup jupyter lab --no-browser --ip=127.0.0.1 --port="${PORT}" \
        --ServerApp.token='' --ServerApp.password='' \
        >"$HOME/.jupyterlab.log" 2>&1 &
  for _ in $(seq 1 30); do curl -s -o /dev/null -m 2 "$URL" && break; sleep 1; done
fi

exec firefox "$URL"
