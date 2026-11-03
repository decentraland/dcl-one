#!/usr/bin/env python3
"""Validate the per-operation backlog; optionally recheck its pinned source inventory.

The default check is self-contained. --upstream additionally derives the surface
from a clean checkout with its lockfile dependencies installed. Neither check is
a claim of behavioural equivalence: review and execution evidence are separate.
"""
import argparse
import hashlib
import json
import re
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "services/catalyst/tests/fixtures/conformance/social/parity.json"
REVISION = "b6b2cefefdf727b2ed47ddb887e77d226a41eea6"
PROTO = "library/protocol/proto/decentraland/social_service/v2/social_service_v2.proto"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def line_at(content, offset):
    return content[:offset].count("\n") + 1


def extract(upstream):
    revision = subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        raise ValueError(f"upstream must be at {REVISION}, got {revision}")
    dirty = subprocess.check_output(["git", "-C", str(upstream), "status", "--porcelain", "--untracked-files=no"], text=True)
    if dirty:
        raise ValueError("upstream tracked files are modified")
    sources = set()
    rows = []

    def ref(path, token="", offset=None):
        sources.add(path)
        content = (upstream / path).read_text()
        at = offset if offset is not None else content.index(token) if token else 0
        return {"path": path, "line": line_at(content, at)}

    def add(kind, method, route, source, handler="", condition="always"):
        rows.append({"id": f"{kind}:{method}:{route}", "kind": kind, "method": method,
                     "name": route, "source": source, "handler": handler, "condition": condition})

    path = "src/controllers/routes/http.routes.ts"
    content = (upstream / path).read_text()
    routes = list(re.finditer(r"router\.(get|post|put|patch|delete)\(\s*['\"]([^'\"]+)", content))
    for i, match in enumerate(routes):
        segment = content[match.end():routes[i+1].start() if i+1 < len(routes) else len(content)]
        handlers = [h for h in re.findall(r"\b(\w+Handler)\b", segment) if h != "IRequestHandler"]
        handler = handlers[-1]
        handler_files = [p for p in (upstream / "src/controllers/handlers/http").glob("*.ts")
                         if re.search(r"(?:function|const)\s+" + handler + r"\b", p.read_text())]
        if len(handler_files) != 1:
            raise ValueError(f"cannot resolve {handler}")
        handler_path = str(handler_files[0].relative_to(upstream))
        ref(handler_path, handler)
        condition = "API_ADMIN_TOKEN configured" if handler in {"getManagedCommunitiesHandler", "getMemberCommunitiesByIdsHandler"} else "always"
        add("api_http", match[1].upper(), match[2], ref(path, offset=match.start()), handler_path, condition)

    rpc_path = "src/controllers/routes/rpc.routes.ts"
    content = (upstream / rpc_path).read_text()
    methods = list(re.finditer(r"^    (\w+): \{", content, re.M))
    for i, match in enumerate(methods):
        segment = content[match.end():methods[i+1].start() if i+1 < len(methods) else len(content)]
        factory = re.search(r"creator:\s*(\w+)", segment)[1]
        files = [p for p in (upstream / "src/controllers/handlers/rpc").glob("*.ts")
                 if re.search(r"function\s+" + factory + r"\b", p.read_text())]
        if len(files) != 1:
            raise ValueError(f"cannot resolve {factory}")
        handler = str(files[0].relative_to(upstream))
        ref(handler, factory)
        streaming = "ServiceType.STREAM" in segment or "ServiceType.COMMUNITIES" in segment
        add("rpc_stream" if streaming else "rpc_unary", "RPC", match[1], ref(rpc_path, match[0]), handler)
        event = re.search(r"event: StreamEvent\.(\w+)", segment)
        if event:
            events = "src/types/subscriptions.ts"
            text = (upstream / events).read_text()
            name = re.search(event[1] + r"\s*=\s*'([^']+)'", text)[1]
            add("json_stream", "SUBSCRIBE", name, ref(events, event[1]), handler)

    uws = "src/controllers/routes/uws.routes.ts"
    add("uws_http", "GET", "/status", ref(uws, "createStatusHandler(components)"), "src/controllers/handlers/uws/status-handler.ts")
    add("uws_http", "GET", "/v1/users/:address/privacy-settings", ref(uws, "createPrivacyHandler(components)"), "src/controllers/handlers/uws/privacy-handler.ts")
    metrics = "node_modules/@dcl/uws-http-server/dist/src/metrics.js"
    add("uws_http", "GET", "/metrics", ref(uws, "createMetricsHandler(components"), metrics, "WKC_METRICS_PUBLIC_PATH overrides path; WKC_METRICS_BEARER_TOKEN may gate access")
    ref(metrics, "async function createMetricsHandler")
    add("uws_http", "ANY", "/health/live", ref(uws, "uwsServer.app.any('/health/live'"))
    add("uws_http", "ANY", "/*", ref(uws, "uwsServer.app.any('/*'"))
    for route, handler in [("/", "src/controllers/handlers/uws/ws-handler.ts"), ("/v1/stream", "src/controllers/handlers/uws/stream-ws-handler.ts")]:
        add("websocket", "UPGRADE", route, ref(handler, "uwsServer.app.ws"), handler)
    for route in ["/health/live", "/health/ready", "/health/startup"]:
        dep = "node_modules/@dcl/http-server/dist/status-checks.js"
        add("api_auxiliary", "GET", route, ref(dep, f"routes.get('{route}'"), dep)
    add("api_auxiliary", "OPTIONS", "/*", ref("src/components.ts", "cors: {"), "node_modules/@dcl/http-server/dist/cors.js")
    add("api_auxiliary", "ANY", "/*", ref("node_modules/@dcl/http-server/dist/server.js", "function handler(request, response)"), "node_modules/@dcl/http-server/dist/server.js")
    for row in rows:
        if row["handler"]:
            ref(row["handler"])
    for source in ["yarn.lock", "src/components.ts", "src/adapters/rpc-server/rpc-server.ts", "src/adapters/stream-sessions/component.ts", "src/adapters/stream-sessions/messages.ts"]:
        ref(source)
    return {"repository": "decentraland/social-service-ea", "revision": revision,
            "counts": dict(sorted(Counter(row["kind"] for row in rows).items())),
            "sources": [{"path": p, "sha256": digest(upstream / p)} for p in sorted(sources)],
            "registrations": sorted(rows, key=lambda row: row["id"])}


def validate(data):
    assert data["schema_version"] == 1
    baseline = data["baseline"]
    assert baseline["revision"] == REVISION
    registrations = baseline["registrations"]
    ids = [row["id"] for row in registrations]
    assert len(ids) == len(set(ids)), "duplicate registrations"
    operations = data["operations"]
    assert set(ids) == {row["id"] for row in operations}, "missing/extra per-operation backlog entry"
    assert len(operations) == len(ids), "duplicate backlog entries"
    expected_counts = dict(sorted(Counter(row["kind"] for row in registrations).items()))
    assert baseline["counts"] == expected_counts, "incorrect inventory counts"
    upstream_rpc = {row["name"][0].upper() + row["name"][1:] for row in registrations if row["kind"].startswith("rpc_")}
    assert upstream_rpc == set(re.findall(r"\brpc\s+(\w+)\s*\(", (ROOT / PROTO).read_text())), "RPC/proto registration set differs"
    for source in baseline["sources"]:
        assert re.fullmatch(r"[0-9a-f]{64}", source["sha256"]), "invalid source digest"
        assert not Path(source["path"]).is_absolute(), "source paths must be repository-relative"
    for row in operations:
        assert row["status"] in {"not_reviewed", "reviewed", "known_gap"}, row["id"]
        assert row["result"] and row["remaining_verification"] and row["owner"], row["id"]
        assert isinstance(row["gaps"], list) and isinstance(row["evidence"], list), row["id"]
        if row["status"] != "not_reviewed":
            assert row["evidence"], f"review needs evidence: {row['id']}"
        if row["status"] == "known_gap":
            assert row["gaps"], f"known gap must name its difference: {row['id']}"
        for evidence in row["evidence"]:
            path = ROOT / evidence["path"]
            assert path.is_file(), f"missing evidence {path}"
            assert not Path(evidence["path"]).is_absolute()
            if evidence.get("test"):
                assert re.search(r"\bfn\s+" + re.escape(evidence["test"]) + r"\b", path.read_text()), f"missing test {evidence}"
            assert evidence["scope"], "evidence must state what it establishes"
    return expected_counts


def report(data):
    counts = data["baseline"]["counts"]
    lines = ["# social-service-ea per-operation parity backlog", "",
             f"Baseline: `decentraland/social-service-ea@{REVISION}`. The machine source is [parity.json](parity.json).",
             "", f"The source inventory contains **{len(data['operations'])} operations**: " + ", ".join(f"{count} {kind}" for kind, count in counts.items()) + ".",
             "", data["review_policy"], "",
             "Source hashes include the pinned service registrations, handlers, stream definitions, lockfile and the installed framework code that supplies health, CORS, metrics and fallback handling. `tooling/social-service-parity.py --upstream <checkout>` re-derives that inventory; the default check validates its complete backlog, local evidence references and the 33-method protocol set without an external checkout.",
             "", "Executed upstream handler cases live in [source-cases.json](source-cases.json); their Protobuf bytes are replayed by `rpc_wire_answers_match_executed_upstream_source_cases`. Those cases use injected dependencies and do not prove deployed HTTP, Redis, SNS, PostgreSQL or live-client equivalence.", ""]
    byid = {row["id"]: row for row in data["operations"]}
    for kind in counts:
        lines += [f"## {kind}", "", "| Operation | Review | Result and remaining work | Evidence |", "| --- | --- | --- | --- |"]
        for reg in data["baseline"]["registrations"]:
            if reg["kind"] != kind:
                continue
            row = byid[reg["id"]]
            evidence = "; ".join(f"`{e['path']}" + (f"::{e['test']}" if e.get("test") else "") + "`" for e in row["evidence"])
            result = row["result"] + " Remaining verification: " + row["remaining_verification"]
            if row["gaps"]:
                result += " Known gaps: " + "; ".join(row["gaps"])
            source = reg["source"]
            source_url = f"https://github.com/decentraland/social-service-ea/blob/{REVISION}/{source['path']}#L{source['line']}" if not source["path"].startswith("node_modules/") else ""
            operation = f"`{reg['method']} {reg['name']}`"
            if source_url:
                operation = f"[{operation}]({source_url})"
            lines.append(f"| {operation} | {row['status']} ({row['owner']}) | {result.replace('|', '&#124;')} | {evidence.replace('|', '&#124;') or 'Pending'} |")
        lines.append("")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument("--upstream", type=Path)
    parser.add_argument("--refresh-inventory", action="store_true", help="rederive pinned registrations, retaining review rows")
    parser.add_argument("--write-report", action="store_true", help="render the readable PARITY.md backlog next to the manifest")
    args = parser.parse_args()
    data = json.loads(args.manifest.read_text()) if args.manifest.exists() else {"schema_version": 1, "operations": []}
    if args.refresh_inventory:
        if not args.upstream:
            parser.error("--refresh-inventory requires --upstream")
        data["baseline"] = extract(args.upstream.resolve())
        existing = {row["id"]: row for row in data["operations"]}
        data["operations"] = [existing.get(row["id"], {
            "id": row["id"], "owner": "unassigned", "status": "not_reviewed",
            "result": "Registered upstream; behaviour has not been audited in this backlog.",
            "evidence": [], "gaps": [],
            "remaining_verification": "Compare handler, domain branches and caller-visible responses; execute differential cases. Route presence does not establish parity."
        }) for row in data["baseline"]["registrations"]]
        args.manifest.write_text(json.dumps(data, indent=2) + "\n")
    counts = validate(data)
    if args.upstream:
        actual = extract(args.upstream.resolve())
        assert data["baseline"] == actual, "upstream registrations or source hashes differ from pinned inventory"
    report_path = args.manifest.with_name("PARITY.md")
    if args.write_report:
        report_path.write_text(report(data))
    if report_path.exists():
        assert report_path.read_text() == report(data), "PARITY.md differs; regenerate with --write-report"
    print(f"social-service inventory: {len(data['operations'])} operations; {json.dumps(counts, sort_keys=True)}")
    print("review statuses: " + json.dumps(dict(Counter(row["status"] for row in data["operations"])), sort_keys=True))
    print("Inventory/evidence validation passed; behavioural parity is not inferred.")


if __name__ == "__main__":
    main()
