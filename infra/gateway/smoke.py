#!/usr/bin/env python3
"""Test the built gateway on an isolated Docker network, without application data."""
import argparse
import base64
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import http.client
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time
import uuid


class Upstream(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_GET(self):
        if self.headers.get("Upgrade", "").lower() == "websocket":
            key = self.headers["Sec-WebSocket-Key"] + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
            self.send_response(101)
            self.send_header("Upgrade", "websocket")
            self.send_header("Connection", "Upgrade")
            self.send_header("Sec-WebSocket-Accept", base64.b64encode(hashlib.sha1(key.encode()).digest()).decode())
            self.end_headers()
            head = self.rfile.read(2)
            length = head[1] & 127
            mask = self.rfile.read(4)
            payload = self.rfile.read(length)
            payload = bytes(byte ^ mask[i % 4] for i, byte in enumerate(payload))
            self.wfile.write(bytes([0x81, len(payload)]) + payload)
            self.wfile.flush()
            self.close_connection = True
            return
        self.rfile.read(int(self.headers.get("Content-Length", "0")))
        if self.path in ("/content/contents/test", "/worlds-content-server/contents/test",
                         "/ipfs/test", "/ab-cdn/content/contents/test"):
            self.send_response(200)
            self.send_header("X-Accel-Redirect", "/__protected_storage/0a/blob")
            self.send_header("ETag", '"gateway-fixture"')
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        body = json.dumps({"upstream": self.server.product, "path": self.path,
                           "headers": {key.lower(): value for key, value in self.headers.items()}}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    do_POST = do_PUT = do_GET


def serve():
    for port, product in [(8081, "apis"), (8082, "sites")]:
        server = ThreadingHTTPServer(("0.0.0.0", port), Upstream)
        server.product = product
        threading.Thread(target=server.serve_forever, daemon=True).start()
    threading.Event().wait()


def request(host, path, *, vhost="example.test", headers=None, method="GET", body=None,
            include_headers=False):
    # Stay below the real production rate budget, even when testing all aliases.
    time.sleep(0.04)
    conn = http.client.HTTPConnection(host, 8080, timeout=10)
    conn.request(method, path, body=body, headers={"Host": vhost, **(headers or {})})
    response = conn.getresponse()
    result = response.status, response.read()
    if include_headers:
        result += ({key.lower(): value for key, value in response.getheaders()},)
    conn.close()
    return result


def websocket(host, path):
    with socket.create_connection((host, 8080), timeout=10) as conn:
        conn.sendall((f"GET {path} HTTP/1.1\r\nHost: example.test\r\nUpgrade: websocket\r\n"
                      "Connection: Upgrade\r\nSec-WebSocket-Version: 13\r\n"
                      "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").encode())
        received = b""
        while b"\r\n\r\n" not in received:
            received += conn.recv(4096)
        assert b" 101 " in received and b"s3pPLMBiTxaQ9kYGzzhZRbK+xOo=" in received, received
        message, mask = b"gateway-echo", b"test"
        conn.sendall(bytes([0x81, 0x80 | len(message)]) + mask
                     + bytes(byte ^ mask[i % 4] for i, byte in enumerate(message)))
        stream = conn.makefile("rb")
        assert stream.read(2) == bytes([0x81, len(message)])
        assert stream.read(len(message)) == message


def wait_ready(host):
    for _ in range(80):
        try:
            if request(host, "/_gateway/healthz")[0] == 200:
                return
        except OSError:
            pass
        time.sleep(0.25)
    raise AssertionError(f"{host} did not become ready")


def client(args):
    table = json.loads(Path(args.table).read_text())
    public, admitted, untrusted, trusted, accel, late = args.targets
    for host in args.targets:
        wait_ready(host)

    # nginx's internal flag alone is insufficient after a host-alias rewrite.
    forged_accel = {"X-Accel-Redirect": "/__protected_storage/0a/blob"}
    for path in ["/peer/__protected_storage/0a/blob", "/worlds/__protected_storage/0a/blob",
                 "/peer/%5f%5fprotected_storage/0a/blob", "/worlds/x/../__protected_storage/0a/blob",
                 "//peer//__protected_storage/0a/blob"]:
        result = request(accel, path, vhost="gateway.example.test", headers=forged_accel)
        assert result[0] == 404, ("external storage alias", path, result)
    for vhost in ["example.test", "peer.example.test", "worlds.example.test", "gateway.example.test",
                  "worlds-content-server.example.test", "abcdn.example.test", "ipfs.example.test"]:
        result = request(accel, "/__protected_storage/0a/blob", vhost=vhost, headers=forged_accel)
        assert result[0] == 404, ("external storage path", vhost, result)
    print("Protected storage rejects direct and rewritten external requests, including forged X-Accel headers.", flush=True)

    def hit(path, product, *, host=public, vhost="example.test", target=None, headers=None, method="GET"):
        status, body = request(host, path, vhost=vhost, headers=headers, method=method)
        assert status == 200, (host, path, status, body)
        data = json.loads(body)
        assert data["upstream"] == product, (path, data)
        assert data["path"] == (target or path), (path, data)
        return data

    for path in ["/", "/play", "/play/engine/pkg/webgpu_build_bg.wasm", "/shop",
                 "/places", "/places.data", "/places/place/1,2", "/places/place/1,2.data",
                 "/places/world/sample.dcl.eth", "/places/world/sample.dcl.eth.data", "/_root.data", "/internal/auth-config",
                 "/internal/thirdweb-sign", "/api/screens/v1/play", "/api/creator-hub/drafts",
                 "/profile/alice", "/assets/example.js", "/docs/index.md"]:
        hit(path, "sites")
    for path in ["/about", "/content/contents/blob", "/lambdas/profiles", "/api/status",
                 "/places/api/status", "/places/api/status.data", "/places/place?position=1,2",
                 "/places/world?name=sample.dcl.eth", "/events/api/status",
                 "/profile-images/alice", "/worlds-content-server/status", "/shop-api/v1/catalog"]:
        hit(path, "apis")
    for alias in table["aliases"]:
        method, path, target = alias["probe"]
        hit(path, "apis", method=method, target=target)
    hit("/place/1,2", "sites", vhost="places.example.test", target="/places/place/1,2")
    hit("/world/sample.dcl.eth", "sites", vhost="places.example.test", target="/places/world/sample.dcl.eth")
    hit("/place?position=1,2", "apis", vhost="places.example.test", target="/places/place?position=1,2")
    print("All root aliases and Sites/API route boundaries passed.", flush=True)

    for path in ["/metrics", "/metrics/", "/metrics/x.data", "/metrics.data",
                 "/METRICS/x.data", "/%6detrics", "/x/../metrics",
                 "/internal/pulse", "/internal/pulse/room-authority/v1", "/internal/pulse/x.data",
                 "/INTERNAL/PULSE/X.data", "//internal//pulse/x.data", "/internal%2fpulse/x.data",
                 "/__protected_storage/0a/blob"]:
        for host in (public, admitted):
            status, body = request(host, path)
            assert status == 404, (path, status, body)
    operator_paths = ["/admin", "/admin/", "/admin.data", "/Admin.data", "/ADMIN/jobs.data",
                      "/admin/api/jobs", "/%61dmin/api/jobs", "/x/../admin/api/jobs",
                      "/server", "/server.data", "/metabase/", "/metabase.data",
                      "/telemetry/dash/sql", "/telemetry/dash/sql.data",
                      "/telemetry/dash/flag.data", "/telemetry/dash/group/target.data",
                      "/telemetry/dash/admin.data", "/telemetry/dash/admin/users"]
    forged = {"X-Forwarded-For": "198.51.100.7", "X-Real-IP": "198.51.100.7",
              "CF-Connecting-IP": "203.0.113.88", "True-Client-IP": "203.0.113.88",
              "X-Catalyst-Admin-Gate": "admitted", "X-Forwarded-Proto": "https",
              "X-Forwarded-Host": "attacker.invalid", "x-original-path": "/admin/api/jobs"}
    for path in operator_paths:
        for host in (public, untrusted):
            assert request(host, path, headers=forged)[0] == 403, path
    hit("/admin/api/jobs", "apis", host=admitted)
    data = hit("/about?x=1%202", "apis", headers=forged)
    assert data["headers"]["x-original-path"] == "/about?x=1%202"
    assert data["headers"]["x-forwarded-host"] == "example.test"
    assert data["headers"]["x-forwarded-proto"] == "http"
    assert data["headers"]["x-forwarded-for"] != "198.51.100.7"
    assert data["headers"]["cf-connecting-ip"] == data["headers"]["x-forwarded-for"]
    assert "true-client-ip" not in data["headers"]
    assert "x-catalyst-admin-gate" not in data["headers"]
    data = hit("/admin/api/jobs", "apis", host=trusted, headers=forged)
    assert data["headers"]["x-forwarded-proto"] == "https"
    assert data["headers"]["x-forwarded-for"] == "198.51.100.7"
    assert data["headers"]["cf-connecting-ip"] == "198.51.100.7"
    assert "true-client-ip" not in data["headers"]
    assert data["headers"]["x-catalyst-admin-gate"] == "admitted"
    assert request(trusted, "/admin/api/jobs", headers={"X-Forwarded-For": "198.51.100.7, 203.0.113.8"})[0] == 403
    print("Internal paths, admin CIDRs, normalized paths and forwarded-header trust passed.", flush=True)

    hit("/about", "apis", vhost="peer.example.test")
    hit("/peer/about?x=1", "apis", vhost="gateway.example.test", target="/about?x=1")
    data = hit("/comms-gatekeeper/get-scene-adapter", "apis", vhost="gateway.example.test")
    assert data["headers"]["x-original-path"] == "/get-scene-adapter"
    hit("/v1/catalog", "apis", vhost="marketplace-api.example.test", target="/marketplace-api/v1/catalog")
    hit("/", "sites", vhost="market.example.test", target="/shop/")
    for path, vhost in [("/values/k", "storage.example.test"), ("/storage/values/k", "gateway.example.test")]:
        data = hit(path, "apis", vhost=vhost, target="/world-storage/values/k")
        assert data["headers"]["x-original-path"] == "/values/k", (vhost, data)
    hit("/world-storage/values/k", "apis", vhost="storage.example.test")
    data = hit("/get-scene-adapter", "apis", vhost="comms-gatekeeper-local.example.test",
        target="/comms-gatekeeper/get-scene-adapter")
    assert data["headers"]["x-original-path"] == "/get-scene-adapter"
    for vhost in ["example.test", "foo.example.test", "gateway.example.test"]:
        for path in ["/a%3Fb", "/peer/a%3fb?x=1"]:
            result = request(public, path, vhost=vhost)
            assert result[0] == 400, ("ambiguous path", vhost, path, result)
    hit("/about?x=a%3Fb", "apis")
    for path, host in [("/", "admin.example.test"), ("/admin/api/jobs", "gateway.example.test")]:
        assert request(public, path, vhost=host, headers=forged)[0] == 403
    for path, host in [("/pulse/x.data", "internal.example.test"),
                       ("/internal/pulse/x.data", "gateway.example.test"),
                       ("/", "metrics.example.test")]:
        assert request(public, path, vhost=host)[0] == 404
    assert request(public, "/", vhost="attacker.invalid")[0] == 404
    assert request(public, "/", vhost="admin.extra.example.test")[0] == 404
    for path in ["/ws", "/social-rpc", "/rpc/mainnet"]:
        websocket(public, path)
    for path, vhost in [
        ("/content/contents/test", "example.test"),
        ("/content/contents/test", "peer.example.test"),
        ("/content/contents/test", "worlds.example.test"),
        ("/peer/content/contents/test", "gateway.example.test"),
        ("/worlds/content/contents/test", "gateway.example.test"),
        ("/contents/test", "worlds-content-server.example.test"),
        ("/test", "ipfs.example.test"),
        ("/content/contents/test", "abcdn.example.test"),
        ("/abcdn/content/contents/test", "gateway.example.test"),
    ]:
        status, body, headers = request(accel, path, vhost=vhost, include_headers=True)
        assert (status, body) == (200, b"authorized-blob"), (vhost, path, status, body)
        for key, value in {"x-content-type-options": "nosniff",
                           "referrer-policy": "strict-origin-when-cross-origin",
                           "cross-origin-opener-policy": "same-origin",
                           "access-control-allow-origin": "*", "etag": '"gateway-fixture"'}.items():
            assert headers.get(key) == value, (vhost, path, key, headers)
    assert request(public, "/about", method="POST", body=b"x" * (1024 * 1024 + 1))[0] == 413
    deployed = request(public, "/entities", vhost="worlds-content-server.example.test", method="POST",
                       body=b"x" * (1024 * 1024 + 1))
    assert deployed[0] == 200, ("worlds deploy body cap", deployed)
    assert json.loads(deployed[1])["path"] == "/worlds-content-server/entities", deployed
    for _ in range(40):
        if request(late, "/about")[0] == 200:
            break
        time.sleep(0.5)
    else:
        raise AssertionError("Gateway did not resolve an upstream that started later")
    hit("/about", "apis", host=late)
    print("Host aliases, signed paths, WebSocket echo, authorized file delivery and body caps passed.", flush=True)
    print("Gateway starts without upstreams and resolves them when they become available.", flush=True)


def docker(*args, check=True):
    return subprocess.run(["docker", *args], check=check, text=True, capture_output=True, timeout=120)


def smoke(args):
    image = args.image
    info = json.loads(docker("image", "inspect", image).stdout)[0]
    assert info["Os"] == "linux" and info["Architecture"] == args.arch, info
    assert info["Config"]["User"] == "1000:1000"
    assert info["Config"]["Labels"]["org.opencontainers.image.revision"] == args.revision
    table = str(Path(info["Config"]["Entrypoint"][0]).parents[1] / "share/gateway/routes.json")
    token = "gateway-smoke-" + uuid.uuid4().hex[:10]
    network, fixture = token, token + "-upstreams"
    containers = []
    logs = Path(".cache/gateway-smoke")
    logs.mkdir(parents=True, exist_ok=True)
    source = Path(__file__).resolve().parent
    common = ["--network", network, "--read-only", "--cap-drop", "ALL",
              "--security-opt", "no-new-privileges", "--tmpfs", "/tmp:rw,nosuid,size=128m"]
    docker("network", "create", "--internal", network)
    try:
        with tempfile.TemporaryDirectory(prefix="gateway-fixture-") as temporary:
            storage = Path(temporary)
            storage.chmod(0o755)
            (storage / "contents/0a").mkdir(parents=True)
            (storage / "contents/0a/blob").write_bytes(b"authorized-blob")
            containers.append(fixture)
            docker("run", "-d", "--name", fixture, *common, "-v", f"{source}:/test:ro",
                   "--entrypoint", "python3", image, "/test/smoke.py", "--serve")
            fixture_ip = json.loads(docker("inspect", fixture).stdout)[0]["NetworkSettings"]["Networks"][network]["IPAddress"]
            cases = [
                ("public", {}),
                ("admitted", {"GATEWAY_ADMIN_CIDRS": fixture_ip + "/32"}),
                ("untrusted", {"GATEWAY_ADMIN_CIDRS": "198.51.100.7/32"}),
                ("trusted", {"GATEWAY_ADMIN_CIDRS": "198.51.100.7/32",
                             "GATEWAY_TRUSTED_PROXY_CIDRS": fixture_ip + "/32"}),
                ("accel", {"GATEWAY_STORAGE_ROOT": "/content"}),
                ("late", {"GATEWAY_APIS_UPSTREAMS": token + "-late-upstreams:8081",
                          "GATEWAY_SITES_UPSTREAMS": token + "-late-upstreams:8082"}),
            ]
            targets = []
            for name, extra in cases:
                container = token + "-" + name
                targets.append(container)
                containers.append(container)
                env = {"GATEWAY_DOMAIN": "example.test", "GATEWAY_APIS_UPSTREAMS": fixture + ":8081",
                       "GATEWAY_SITES_UPSTREAMS": fixture + ":8082", **extra}
                env_args = [item for key, value in env.items() for item in ("-e", f"{key}={value}")]
                docker("run", "-d", "--name", container, *common, *env_args, "-v", f"{storage}:/content:ro", image)
            docker("exec", fixture, "python3", "/test/smoke.py", "--unresolved", targets[-1])
            late_fixture = token + "-late-upstreams"
            containers.append(late_fixture)
            docker("run", "-d", "--name", late_fixture, *common, "-v", f"{source}:/test:ro",
                   "--entrypoint", "python3", image, "/test/smoke.py", "--serve")
            result = docker("exec", fixture, "python3", "/test/smoke.py", "--client",
                            "--table", table, "--targets", *targets, check=False)
            (logs / f"{token}-probes.log").write_text(result.stdout + result.stderr)
            print(result.stdout, end="")
            if result.returncode:
                raise AssertionError(result.stderr)
            print(f"Gateway image smoke passed: linux/{args.arch}, revision {args.revision}")
    finally:
        for container in reversed(containers):
            result = docker("logs", container, check=False)
            (logs / f"{container}.log").write_text(result.stdout + result.stderr)
            docker("rm", "-f", container, check=False)
        docker("network", "rm", network, check=False)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serve", action="store_true")
    parser.add_argument("--client", action="store_true")
    parser.add_argument("--unresolved")
    parser.add_argument("--table")
    parser.add_argument("--targets", nargs=6)
    parser.add_argument("--image", default="foundation-gateway:nix")
    parser.add_argument("--revision")
    parser.add_argument("--arch", choices=("amd64", "arm64"))
    args = parser.parse_args()
    if args.serve:
        serve()
    elif args.client:
        client(args)
    elif args.unresolved:
        wait_ready(args.unresolved)
        assert request(args.unresolved, "/about")[0] == 502
    else:
        if not args.revision or not args.arch:
            parser.error("--revision and --arch are required")
        smoke(args)
