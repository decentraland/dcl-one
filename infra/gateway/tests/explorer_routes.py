"""Compare real Explorer/HUD bases with the rendered nginx host maps.

Called by common's ignored gateway_routes_match_deployment test with Rust outputs on stdin.
Requires Python 3 and Node.js 24+; the HUD runs with Node's built-in TypeScript loader.
"""
import json
from pathlib import Path
import subprocess
import sys
from urllib.parse import urlsplit

from test_routing import route


def check(rows, root):
    # Only the browser location and the generated barrel import need a harness. Both the
    # service table and serviceUrl/engineServiceOptions execute their production source.
    result = subprocess.run(["node", "--input-type=module", "-e", """
        import { registerHooks } from 'node:module';
        import { pathToFileURL } from 'node:url';
        import { resolve } from 'node:path';
        const root = process.argv[1];
        const play = resolve(root, 'apps/ui3/src/play');
        const generated = resolve(root, 'library/explorer-core/engine/generated');
        registerHooks({ resolve(specifier, context, next) {
            if (specifier === '@explorer-core/engine/generated') {
                return { url: pathToFileURL(resolve(generated, 'serviceTable.ts')).href, shortCircuit: true };
            }
            return next(specifier, context);
        }});
        globalThis.window = { location: new URL('https://example.test/play'), __DCL_PUBLIC__: {} };
        const hud = await import(pathToFileURL(resolve(play, 'lib/baseDomain.ts')));
        const { SERVICES } = await import(pathToFileURL(resolve(generated, 'serviceTable.ts')));
        console.log(JSON.stringify({ services: SERVICES, options: hud.engineServiceOptions() }));
    """, str(root)], text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
    hud = json.loads(result.stdout)
    web = {row["name"]: row for row in hud["services"]}
    assert set(web) == {row["name"] for row in rows if row["web"]}, "HUD service table is stale"
    # Not served by the gateway yet, so the engine and the web page both reach the public host
    # (decentraland/dcl-one#631): abgen's CDN, whose Bevy variants `/ab-cdn` refuses.
    ungated = {"abgenCdn"}
    excluded = {"authPage", "reels", "storage", "ethereumRpc", "socialRpc", "pulseServer"} | ungated
    assert excluded <= {row["name"] for row in rows}, "removed service needs contract review"

    for row in rows:
        name = row["name"]
        assert (row["direct"] is None) == (name in excluded), row
        assert (row["vhost"] is None) == (name in excluded), row
        # Exclusions retain the real resolver's base-domain composition, including ws(s)
        # RPC schemes, Pulse's authority, and the public auth/reels page suffixes.
        host = f'{row["sub"]}.decentraland.org' if row["sub"] else "decentraland.org"
        fallback = f'{row["scheme"]}://{host}{row["path"]}' if row["scheme"] else host
        assert row["fallback"] == fallback, row
        if row["web"]:
            assert web[name] == {key: row[key] for key in ("name", "value", "scheme", "sub", "path")}, row
            hud_base = hud["options"][name]
            if name == "pulseServer":
                assert hud_base == "example.test", hud_base
            elif name in ungated:
                # composed from Decentraland's domain, never from the page's own host
                assert hud_base == f'https://{row["sub"]}.decentraland.org{row["path"]}', hud_base
            else:
                expected_scheme = "wss" if row["value"] == "websocket" else "https"
                assert urlsplit(hud_base).scheme == expected_scheme, hud_base
                # The web page uses apex routes for sockets, storage and site links too.
                assert urlsplit(hud_base).netloc == "example.test", hud_base
                if name in excluded:
                    actual, _ = route(f'{row["sub"]}.example.test' if row["sub"] else "example.test", row["path"] or "/")
                    assert actual.rstrip("/") == urlsplit(hud_base).path, (name, actual, hud_base)
        if name in excluded:
            continue

        direct, virtual = urlsplit(row["direct"]), urlsplit(row["vhost"])
        assert direct.scheme == virtual.scheme == "https", row
        assert direct.netloc == "example.test", row
        assert virtual.netloc == "gateway.example.test", row
        virtual_path, _ = route(virtual.hostname, virtual.path + "/child")
        if name == "assetsCdn":
            # Catalyst exposes thumbnails under /social-api/social/communities/... .
            # nginx's root-aliases.nix additionally admits /assets-cdn/social/communities/...;
            # served_routes_tests covers the root and /social-api backend forms.
            thumbnail = "/social/communities/00000000-0000-0000-0000-000000000000/raw-thumbnail.png"
            assert row["direct"] == hud["options"]["socialApi"], row
            assert direct.path + thumbnail == "/social-api" + thumbnail, row
            assert virtual.path + thumbnail == "/assets-cdn" + thumbnail, row
            assert virtual_path == urlsplit(hud_base).path + "/child", row
        else:
            assert virtual_path == direct.path + "/child", row
            if row["web"]:
                assert row["direct"] == hud_base, (row, hud_base)
        for probe in row["signing"]:
            target = urlsplit(probe["target"])
            request_uri = target.path + ("?" + target.query if target.query else "")
            _, original = route(target.hostname, request_uri)
            # Signed fetch binds the path; nginx forwards the query too, which the
            # backend discards when rebuilding the signature payload.
            assert probe["signed"] == original.split("?")[0], (name, probe, original)


if __name__ == "__main__":
    check(json.load(sys.stdin), Path(sys.argv[1]))
