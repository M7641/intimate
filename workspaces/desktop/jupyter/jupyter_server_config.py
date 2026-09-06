# Adds a "Desktop" button to the Jupyter launcher (jupyter-server-proxy), wired to KasmVNC.
#
# KasmVNC serves a relative-path web client (so it works fine under the /user/<name>/desktop/
# prefix) BUT it requires HTTP Basic Auth — without it the proxied request gets 401 and the
# page is blank. So we inject an Authorization header on every proxied request, using the same
# internal credential that start-desktop sets on KasmVNC (KASM_DESKTOP_PW, default below).
# The real access gate is JupyterHub's own login; this Basic Auth is just KasmVNC's internal
# handshake, kept fixed/valid-length so KasmVNC never falls back to its interactive prompt.
import base64
import getpass
import os

_user = os.environ.get("WORKSPACE_USER_ID") or getpass.getuser()
_pw = os.environ.get("KASM_DESKTOP_PW", "kasmworkspace")
_auth = base64.b64encode(f"{_user}:{_pw}".encode()).decode()

c = get_config()  # noqa: F821
c.ServerProxy.servers = {
    "desktop": {
        "command": ["/usr/bin/start-desktop", "{port}"],
        "port": 6901,
        "timeout": 90,
        "absolute_url": False,
        # KasmVNC needs Basic Auth; jupyter-server-proxy injects it on HTTP + websocket.
        "request_headers_override": {"Authorization": f"Basic {_auth}"},
        "launcher_entry": {"title": "Desktop", "enabled": True},
    },
}
