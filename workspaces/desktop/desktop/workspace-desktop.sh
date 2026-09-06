#!/usr/bin/env bash
#
# workspace-desktop — DESKTOP-FIRST entrypoint for the Nimbus workspace.
#
# Serves an XFCE desktop via KasmVNC on the platform port, authenticated with the
# credentials Nimbus injects (WORKSPACE_USER_ID / WORKSPACE_USER_PASSWORD).
#
# This REPLACES the JupyterHub entrypoint. Consequences:
#   * No JupyterHub -> no /hub/login -> the IP-bound XSRF 403 cannot occur.
#   * JupyterLab / code-server / Zed are still installed; launch them from inside
#     the desktop (terminal or menu) instead of as the entrypoint.
#
# Platform contract (decoded from the Nimbus helm chart + addUser.sh):
#   * the OS user $WORKSPACE_USER_ID is created by the postStart hook (we wait for it);
#   * /home/$WORKSPACE_USER_ID is the persistent volume;
#   * readiness is a TCP probe on the container port — just listen on it.
#
# You authenticate to KasmVNC with WORKSPACE_USER_ID / WORKSPACE_USER_PASSWORD (the same
# credentials Nimbus assigns) — KasmVNC must own a user, and these are the ones you know.
#
set -uo pipefail

PORT="${WS_DESKTOP_PORT:-8000}"               # Istio routes the workspace port here
WS_USER="${WORKSPACE_USER_ID:-nimbus}"
WS_PASS="${WORKSPACE_USER_PASSWORD:-changeme}"
HOME_DIR="/home/${WS_USER}"

echo "[desktop] waiting for platform-created user '${WS_USER}'..."
for _ in $(seq 1 30); do id "${WS_USER}" >/dev/null 2>&1 && break; sleep 1; done
if ! id "${WS_USER}" >/dev/null 2>&1; then
  echo "[desktop] user not present yet; creating fallback"
  useradd -m -d "${HOME_DIR}" -s /bin/bash "${WS_USER}" || true
fi

# Seed default desktop config (Materia-dark theme, code-server settings, …) into the
# persistent home on first run. The PV is mounted pre-existing, so useradd's /etc/skel copy
# is skipped — without this the theming never applies. cp -n never clobbers the user's files.
cp -rn /etc/skel/. "${HOME_DIR}/" 2>/dev/null || true
chown -R "${WS_USER}:${WS_USER}" "${HOME_DIR}/.config" "${HOME_DIR}/.local" 2>/dev/null || true

mkdir -p "${HOME_DIR}/.vnc"

# VNC session start. Desktop is selectable via WORKSPACE_DE (default: KDE Plasma; set
# WORKSPACE_DE=xfce to fall back). French AZERTY by default (override with KEYBOARD_LAYOUT).
# Plasma/KWin need a valid XDG_RUNTIME_DIR; compositing is off (no GPU, streamed over VNC).
# No custom xstartup: KasmVNC's -select-de (below) writes the correct one for the chosen DE
# and a custom xstartup would be bypassed anyway. Keyboard (AZERTY) is set per-DE via config
# (desktop/kde/kxkbrc for Plasma).

# Serve plain HTTP on the platform port, bound to all interfaces so Nimbus's TCP probe and
# Istio can reach it. websocket_port MUST be a fixed integer ("auto" -> 8443+display, so
# nothing would listen on ${PORT}).
cat > "${HOME_DIR}/.vnc/kasmvnc.yaml" <<EOF
network:
  protocol: http
  interface: 0.0.0.0
  websocket_port: ${PORT}
  ssl:
    require_ssl: false
EOF
chown -R "${WS_USER}:${WS_USER}" "${HOME_DIR}/.vnc" 2>/dev/null || true

# KasmVNC credential -> default store (~/.kasmpasswd) with -w, written AS the user. Fail
# LOUD if it doesn't take (e.g. WORKSPACE_USER_PASSWORD < 6 chars, which KasmVNC rejects)
# instead of letting kasmvncserver fall into its interactive prompt and hang the probe.
if ! runuser -u "${WS_USER}" -- bash -c \
     'printf "%s\n%s\n" "$1" "$1" | kasmvncpasswd -u "$2" -w >/dev/null 2>&1 && [ -s "$HOME/.kasmpasswd" ]' \
     _ "${WS_PASS}" "${WS_USER}"; then
  echo "[desktop] FATAL: could not set KasmVNC password for '${WS_USER}' (is WORKSPACE_USER_PASSWORD >=6 chars?). Aborting so the failure is visible." >&2
  exit 1
fi

# X11/ICE socket dirs (Plasma's session manager needs /tmp/.ICE-unix).
mkdir -p /tmp/.X11-unix /tmp/.ICE-unix && chmod 1777 /tmp/.X11-unix /tmp/.ICE-unix

# Desktop = XFCE via KasmVNC's -select-de. The flag MUST be passed — without it the wrapper
# prompts for a DE interactively and exits before binding the web port. (Only xfce.desktop is
# installed; KDE was removed — it can't start on this headless, GPU-less Xvnc.)
KASM_DE=xfce

echo "[desktop] starting KasmVNC desktop (${KASM_DE}) on port ${PORT} as ${WS_USER}"
runuser -u "${WS_USER}" -- bash -lc "
  export XDG_RUNTIME_DIR=/tmp/runtime-\$(id -u)
  export KWIN_COMPOSE=N
  mkdir -p \"\$XDG_RUNTIME_DIR\" && chmod 700 \"\$XDG_RUNTIME_DIR\"
  exec kasmvncserver :1 -select-de ${KASM_DE} -geometry 1920x1080
" >/dev/null 2>&1 \
  || echo "[desktop] kasmvncserver exited non-zero — inspect ${HOME_DIR}/.vnc/*.log" >&2

# Fail-fast visibility: confirm something answers on the port (a 401 counts — KasmVNC is up).
sleep 2
if curl -s -o /dev/null -m 4 "http://127.0.0.1:${PORT}/"; then
  echo "[desktop] KasmVNC XFCE up on port ${PORT} (display :1)"
else
  echo "[desktop] WARNING: nothing answering on port ${PORT} yet — see ${HOME_DIR}/.vnc/*.log" >&2
fi

# Keep PID 1 alive and surface the server log.
exec tail -F "${HOME_DIR}"/.vnc/*.log 2>/dev/null || sleep infinity
