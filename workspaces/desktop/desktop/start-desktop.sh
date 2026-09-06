#!/usr/bin/env bash
#
# start-desktop [PORT] — launch a browser-accessible XFCE desktop via KasmVNC.
#
# Two callers:
#   * the Jupyter launcher "Desktop" button (jupyter-server-proxy passes port 6901),
#   * directly in a terminal (`start-desktop`, defaults to port 6080) — the
#     Jupyter-independent path, and the basis for one day moving off JupyterHub.
#
# KasmVNC serves its own web client (no separate noVNC/websockify). The container
# must run with: --security-opt seccomp=unconfined
#
# Auth: KasmVNC requires a user/password (it returns 401 otherwise). We set one
# non-interactively in ~/.kasmpasswd. For the Jupyter "Desktop" button this exact
# credential is injected as Basic Auth by jupyter/jupyter_server_config.py, so the two
# MUST agree — both default to KASM_DESKTOP_PW (>=6 chars; KasmVNC rejects shorter ones
# and would then drop into an interactive prompt and hang). The real gate is Jupyter login.
#
set -euo pipefail

PORT="${1:-6080}"
DISPLAY_NUM="${DISPLAY_NUM:-1}"
GEOMETRY="${GEOMETRY:-1360x768}"
VNC_USER="${VNC_USER:-$(id -un)}"
VNC_PW="${VNC_PW:-${KASM_DESKTOP_PW:-kasmworkspace}}"

mkdir -p "$HOME/.vnc"

# XFCE session for the VNC display.
cat > "$HOME/.vnc/xstartup" <<'EOF'
#!/bin/sh
unset SESSION_MANAGER DBUS_SESSION_BUS_ADDRESS
export XDG_CURRENT_DESKTOP=XFCE
exec dbus-launch --exit-with-session xfce4-session
EOF
chmod +x "$HOME/.vnc/xstartup"

# Per-session config: serve plain HTTP on the requested port, bound to all interfaces
# (so the platform probe / proxy can reach it). websocket_port MUST be a fixed integer
# — "auto" would resolve to 8443+display and nothing would be listening on ${PORT}.
cat > "$HOME/.vnc/kasmvnc.yaml" <<EOF
network:
  protocol: http
  interface: 0.0.0.0
  websocket_port: ${PORT}
  ssl:
    require_ssl: false
EOF

# Non-interactive credential -> default store (~/.kasmpasswd) with -w. Fail LOUD if it
# doesn't take (e.g. password too short) instead of letting kasmvncserver hang on a prompt.
if ! printf '%s\n%s\n' "$VNC_PW" "$VNC_PW" | kasmvncpasswd -u "$VNC_USER" -w >/dev/null 2>&1 \
   || [ ! -s "$HOME/.kasmpasswd" ]; then
  echo "[start-desktop] FATAL: kasmvncpasswd failed for '$VNC_USER' (password <6 chars?). Aborting." >&2
  exit 1
fi

# (Re)start KasmVNC on :DISPLAY_NUM with an XFCE session.
kasmvncserver -kill ":${DISPLAY_NUM}" >/dev/null 2>&1 || true
kasmvncserver ":${DISPLAY_NUM}" -geometry "${GEOMETRY}" -select-de xfce

# Fail-fast visibility: confirm something is actually answering on the port (a 401 counts
# — it means KasmVNC is up). Connection-refused here is the silent-hang symptom.
sleep 2
if curl -s -o /dev/null -m 4 "http://127.0.0.1:${PORT}/"; then
  echo "[start-desktop] KasmVNC XFCE up on port ${PORT} (display :${DISPLAY_NUM})"
else
  echo "[start-desktop] WARNING: nothing answering on port ${PORT} yet — see ~/.vnc/*.log" >&2
fi

# Stay in the foreground so jupyter-server-proxy (or a terminal) keeps it alive; surface logs.
exec tail -F "$HOME"/.vnc/*.log 2>/dev/null || sleep infinity
