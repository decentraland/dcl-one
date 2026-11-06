#!/usr/bin/env python3
"""Serves the preview page (web/) for local iteration: every response carries `Cache-Control:
no-cache`, so a reload always revalidates the module and the page (a plain file server lets the
browser keep a rebuilt module for a while), plus the headers an embedding page needs.

    python3 tools/serve.py [port]   (default 5300, loopback only)
"""
import sys
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

WEB = Path(__file__).resolve().parent.parent / "web"


class Handler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".mjs": "text/javascript"}

    def end_headers(self):
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Cross-Origin-Resource-Policy", "cross-origin")
        super().end_headers()

    def log_message(self, fmt, *args):
        sys.stderr.write("%s %s\n" % (self.log_date_time_string(), fmt % args))


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 5300
    server = ThreadingHTTPServer(("127.0.0.1", port), partial(Handler, directory=str(WEB)))
    print(f"serving {WEB} on http://localhost:{port}/ (no-cache)", flush=True)
    server.serve_forever()
