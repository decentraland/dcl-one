#!/usr/bin/env python3
"""Serves the preview page (web/) for local iteration: every response carries `Cache-Control:
no-cache`, so a reload always revalidates the module and the page (a plain file server lets the
browser keep a rebuilt module for a while), plus the headers an embedding page needs.

    python3 tools/serve.py [port]   (default 5300, loopback only)

It listens on both loopback addresses: browsers resolve `localhost` to `::1` first, and a
server on `127.0.0.1` alone makes a page load fail now and then.
"""
import socket
import sys
import threading
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


class Server(ThreadingHTTPServer):
    daemon_threads = True
    # a page and its module open several connections at once
    request_queue_size = 64


class Server6(Server):
    address_family = socket.AF_INET6


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 5300
    handler = partial(Handler, directory=str(WEB))
    servers = [Server(("127.0.0.1", port), handler)]
    try:
        servers.append(Server6(("::1", port), handler))
    except OSError as e:
        print(f"no IPv6 loopback ({e}); serving 127.0.0.1 only", flush=True)
    print(f"serving {WEB} on http://localhost:{port}/ (no-cache)", flush=True)
    for server in servers[1:]:
        threading.Thread(target=server.serve_forever, daemon=True).start()
    servers[0].serve_forever()
