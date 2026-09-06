"""A tiny local API with an OpenAPI spec, used to exercise the API reflector.

Serves:
  GET /openapi.json     the spec (discovery target)
  GET /health           {"status": "ok", ...}
  GET /orders/{id}       200 for a numeric id, 404 for 999999, 400 for non-numeric

No external calls — bind to localhost and throw away. Run:
    python3 tests/fixture_api_server.py 8791
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

OPENAPI = {
    "openapi": "3.0.0",
    "info": {"title": "orders-fixture", "version": "1.0.0"},
    "paths": {
        "/health": {"get": {"summary": "Liveness probe"}},
        "/orders/{id}": {"get": {"summary": "Fetch one order by id"}},
    },
}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):  # keep test output quiet
        pass

    def _send(self, status, body):
        payload = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def do_GET(self):
        if self.path == "/openapi.json":
            return self._send(200, OPENAPI)
        if self.path == "/whoami":
            # Echo auth headers back — lets a sample plan verify auth passing.
            return self._send(
                200,
                {
                    "authorization": self.headers.get("Authorization"),
                    "x_api_key": self.headers.get("X-Api-Key"),
                },
            )
        if self.path == "/health":
            return self._send(200, {"status": "ok", "version": "1.2.3"})
        if self.path.startswith("/orders/"):
            raw = self.path[len("/orders/") :]
            if not raw.isdigit():
                return self._send(400, {"error": "id must be numeric", "got": raw})
            if raw == "999999":
                return self._send(404, {"error": "order not found", "id": 999999})
            return self._send(200, {"id": int(raw), "total": 42.5, "currency": "GBP"})
        return self._send(404, {"error": "not found"})


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8791
    HTTPServer(("127.0.0.1", port), Handler).serve_forever()
