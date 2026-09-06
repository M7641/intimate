#!/usr/bin/env bash
#
# start-tunnel-desktop — start the VS Code (or Cursor) remote tunnel from the desktop.
#
# Launched from the desktop icon / app menu, inside a terminal so the GitHub device-login
# link and code are visible. `start-tunnel` (vscode-tunnel/scripts) spawns the tunnel
# detached and prints the link; we surface it, open the login page in Firefox, then tail
# the log so the status (and the eventual vscode.dev URL) stays on screen. Closing this
# window leaves the tunnel running.
#
set -uo pipefail

APP="${1:-code}"
LOG="/vscode-tunnel/logs/output.log"

echo "==============================================="
echo "  Starting ${APP^} remote tunnel…"
echo "==============================================="
echo

# Starts the tunnel (detached) and prints the "Open this link in your browser" block.
start-tunnel "$APP"

echo
echo "-----------------------------------------------"
echo "  1. A browser opened at https://github.com/login/device"
echo "  2. Enter the code shown above to authorise this device."
echo "  3. Then connect from your local editor:"
echo "       VS Code  → 'Remote Tunnels: Connect to Tunnel…'"
echo "       Browser  → https://vscode.dev  (pick this machine)"
echo "-----------------------------------------------"
echo

# Convenience: open the device-login page so the code can be pasted straight in.
(command -v firefox >/dev/null 2>&1 && nohup firefox "https://github.com/login/device" >/dev/null 2>&1 &) || true

echo "Live tunnel log below. Close this window to keep the tunnel running."
echo "(Use 'stop-tunnel ${APP}' in a terminal to stop it.)"
echo
exec tail -n +1 -F "$LOG"
