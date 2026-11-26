#!/usr/bin/env python3
"""Serves the preview page (web/) for local iteration: every response carries `Cache-Control:
no-cache`, so a reload always revalidates the module and the page (a plain file server lets the
browser keep a rebuilt module for a while), plus the headers an embedding page needs.

    python3 tools/serve.py [port]                 (default 5300, loopback only)
    python3 tools/serve.py [port] --dist <dir>    a tools/package.py output, as Sites serves it:
                                                  the page at /avatar-preview/ (revalidated), its
                                                  files at /assets/avatar-preview/ (immutable)

It listens on both loopback addresses: browsers resolve `localhost` to `::1` first, and a
server on `127.0.0.1` alone makes a page load fail now and then.
"""
import argparse
import socket
import sys
import threading
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

WEB = Path(__file__).resolve().parent.parent / "web"
PAGE = "/avatar-preview/"
ASSETS = "/assets/avatar-preview/"


class Handler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".mjs": "text/javascript",
                      ".glb": "model/gltf-binary", ".otf": "font/otf"}
    dist = False

    def end_headers(self):
        if not self.dist:
            self.send_header("Cache-Control", "no-cache")
        elif urlsplit(self.path).path.startswith(ASSETS):
            # react-router-serve's /assets/
            self.send_header("Cache-Control", "public, max-age=31536000, immutable")
        else:
            # react-router-serve's default for every other client file
            self.send_header("Cache-Control", "public, max-age=0")
        self.send_header("Cross-Origin-Resource-Policy", "cross-origin")
        super().end_headers()

    def translate_path(self, path):
        if not self.dist:
            return super().translate_path(path)
        route = urlsplit(path).path
        if route == "/avatar-preview":
            return str(Path(self.directory) / "missing")
        if route.startswith(PAGE):
            return super().translate_path(route[len(PAGE) - 1:])
        if route.startswith(ASSETS):
            return super().translate_path("/assets/" + route[len(ASSETS):])
        return str(Path(self.directory) / "missing")

    def send_head(self):
        if self.dist and urlsplit(self.path).path == "/avatar-preview":
            self.send_response(301)
            self.send_header("Location", PAGE)
            self.end_headers()
            return None
        return super().send_head()

    def log_message(self, fmt, *args):
        sys.stderr.write("%s %s\n" % (self.log_date_time_string(), fmt % args))


class Server(ThreadingHTTPServer):
    daemon_threads = True
    # a page and its module open several connections at once
    request_queue_size = 64


class Server6(Server):
    address_family = socket.AF_INET6


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("port", type=int, nargs="?", default=5300)
    parser.add_argument("--dist", type=Path, help="a tools/package.py output to serve as Sites does")
    args = parser.parse_args()
    root = args.dist.resolve() if args.dist else WEB
    handler = partial(type("Served", (Handler,), {"dist": bool(args.dist)}), directory=str(root))
    servers = [Server(("127.0.0.1", args.port), handler)]
    try:
        servers.append(Server6(("::1", args.port), handler))
    except OSError as e:
        print(f"no IPv6 loopback ({e}); serving 127.0.0.1 only", flush=True)
    where = f"http://localhost:{args.port}{PAGE if args.dist else '/'}"
    print(f"serving {root} on {where} ({'as Sites does' if args.dist else 'no-cache'})", flush=True)
    for server in servers[1:]:
        threading.Thread(target=server.serve_forever, daemon=True).start()
    servers[0].serve_forever()
