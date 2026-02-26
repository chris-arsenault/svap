"""
Local development server that wraps the Lambda handler in an HTTP server.

Usage:
    python -m svap.dev_server          # port 8000
    python -m svap.dev_server 3000     # custom port

The Vite dev server proxies /api/* to this server.
"""

import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse

# Parameterized routes — order doesn't matter, checked by segment count + prefix
_PARAM_ROUTES = [
    ("GET", "/api/cases/", "GET /api/cases/{case_id}", "case_id"),
    ("GET", "/api/taxonomy/", "GET /api/taxonomy/{quality_id}", "quality_id"),
    ("GET", "/api/policies/", "GET /api/policies/{policy_id}", "policy_id"),
]


def _resolve_route(method, path):
    """Match a request path to a routeKey and extract path parameters."""
    clean = path.rstrip("/")

    for route_method, prefix, route_key, param_name in _PARAM_ROUTES:
        if method == route_method and clean.startswith(prefix.rstrip("/")):
            # e.g. /api/cases/abc123 → segments after prefix
            remainder = clean[len(prefix.rstrip("/")) :]
            if remainder.startswith("/") and "/" not in remainder[1:]:
                return route_key, {param_name: remainder[1:]}

    return f"{method} {clean}", {}


class DevHandler(BaseHTTPRequestHandler):
    """Translates HTTP requests into API Gateway V2 events."""

    def _handle(self, method):
        from svap.api import handler

        parsed = urlparse(self.path)
        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length).decode() if content_length else ""

        route_key, path_params = _resolve_route(method, parsed.path)

        event = {
            "version": "2.0",
            "routeKey": route_key,
            "rawPath": parsed.path,
            "headers": {k.lower(): v for k, v in self.headers.items()},
            "pathParameters": path_params or None,
            "body": body or None,
            "isBase64Encoded": False,
            "requestContext": {"http": {"method": method, "path": parsed.path}},
        }

        result = handler(event, None)

        self.send_response(result["statusCode"])
        for k, v in result.get("headers", {}).items():
            self.send_header(k, v)
        # Add CORS for local dev (API Gateway handles this in production)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Headers", "*")
        self.send_header("Access-Control-Allow-Methods", "*")
        self.end_headers()
        self.wfile.write(result.get("body", "").encode())

    def do_GET(self):
        self._handle("GET")

    def do_POST(self):
        self._handle("POST")

    def do_OPTIONS(self):
        """Handle CORS preflight for local dev."""
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Headers", "*")
        self.send_header("Access-Control-Allow-Methods", "*")
        self.send_header("Access-Control-Max-Age", "600")
        self.end_headers()

    def log_message(self, format, *args):
        status = args[1] if len(args) > 1 else ""
        sys.stderr.write(f"  {status} {args[0]}\n")


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
    server = HTTPServer(("0.0.0.0", port), DevHandler)
    print(f"SVAP dev server on http://localhost:{port}")
    print(f"Health check: http://localhost:{port}/api/health")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopped.")


if __name__ == "__main__":
    main()
