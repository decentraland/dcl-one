import importlib.util
from pathlib import Path
import re
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("gateway", Path(__file__).parents[1] / "gateway.py")
gateway = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gateway)


class ConfigurationTest(unittest.TestCase):
    def test_rejects_configuration_injection_before_starting_nginx(self):
        bad = {
            "GATEWAY_DOMAIN": ["example.org;\ninclude /tmp/x", "*.example.org", "https://example.org", "a..b"],
            "GATEWAY_HOST_ALIASES": ["example.org;root /"],
            "GATEWAY_APIS_UPSTREAMS": ["http://apis:5210", "apis:0", "apis:65536", "apis:5210;"],
            "GATEWAY_SITES_UPSTREAMS": ["unix:/tmp/socket", "sites:5212/path"],
            "GATEWAY_ADMIN_CIDRS": ["allow all;", "example.org", "203.0.113.1/33", "fe80::1%eth0/128"],
            "GATEWAY_TRUSTED_PROXY_CIDRS": ["0.0.0.0/0;"],
            "GATEWAY_TRUSTED_SYNC_PEER_IPS": ["203.0.113.1;", "peer.example", "fe80::1%eth0"],
            "GATEWAY_RESOLVERS": ["127.0.0.1;include", "resolver.example", "fe80::1%eth0"],
            "GATEWAY_STORAGE_ROOT": ["relative", "/data;include", "/data\nroot"],
        }
        for key, values in bad.items():
            for value in values:
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1", key: value})

    def test_admin_and_forwarded_identity_are_closed_by_default(self):
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1"})
        self.assertEqual(cfg["admins"], [])
        self.assertEqual(cfg["trusted"], [])
        self.assertEqual(cfg["storage"], "")

    def test_canonicalizes_networks_and_supports_multiple_upstreams(self):
        cfg = gateway.settings({
            "GATEWAY_RESOLVERS": "::1,127.0.0.1",
            "GATEWAY_ADMIN_CIDRS": "203.0.113.17/24,2001:db8::1/64",
            "GATEWAY_APIS_UPSTREAMS": "apis-a:5210,[::1]:5211",
        })
        self.assertEqual(cfg["admins"], ["203.0.113.0/24", "2001:db8::/64"])
        self.assertEqual(cfg["apis"], ["apis-a:5210", "[::1]:5211"])
        self.assertEqual(cfg["resolvers"], "[::1] 127.0.0.1")

    def test_sync_peers_get_an_uncounted_limit_key(self):
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1",
                                "GATEWAY_TRUSTED_SYNC_PEER_IPS": "203.0.113.7, 2001:db8::1"})
        self.assertEqual(cfg["sync_peers"], ["203.0.113.7/32", "2001:db8::1/128"])
        text = gateway.render({"sites": [], "internal": [], "node": [], "aliases": [], "deploy": []},
                              cfg, "/mime.types")
        self.assertIn("geo $sync_peer { default 0; 203.0.113.7/32 1; 2001:db8::1/128 1; }", text)
        self.assertIn("limit_req_zone $binary_remote_addr zone=read:10m rate=30r/s;", text)
        self.assertIn("limit_req_zone $content_limit_key zone=content:10m rate=100r/s;", text)
        key_map = re.search(r'map "\$sync_peer:\$request_method:\$uri" \$content_limit_key \{(.*?)\}', text, re.S)[1]
        exempt = re.search(r'"(~[^"]+)" ""', key_map)[1][1:]
        # Upstream exempts sync peers on GET /contents/{hash} only.
        for subject in ["1:GET:/content/contents/bafy1", "1:HEAD:/contents/bafy1", "1:GET:/contents/bafy1/"]:
            with self.subTest(subject=subject):
                self.assertTrue(re.search(exempt, subject))
        for subject in ["0:GET:/content/contents/bafy1", "1:POST:/content/contents/bafy1",
                        "1:GET:/content/entities/active", "1:GET:/content/contents/bafy1/active-entities",
                        "1:GET:/worlds-content-server/contents/bafy1", "1:GET:/lambdas/profiles"]:
            with self.subTest(subject=subject):
                self.assertIsNone(re.search(exempt, subject))
        self.assertIn("limit_req_zone $binary_remote_addr zone=deploy:10m rate=2r/s;", text)

    def test_uses_container_dns_without_a_fixed_host_address(self):
        with tempfile.TemporaryDirectory() as folder:
            resolv = Path(folder) / "resolv.conf"
            resolv.write_text("nameserver 127.0.0.1\nnameserver ::1\n")
            self.assertEqual(gateway.settings({}, resolv)["resolvers"], "127.0.0.1 [::1]")

    def test_sites_routes_keep_api_exceptions_and_prefix_boundaries(self):
        self.assertEqual(gateway.site_prefixes([
            "_index.tsx", "$slug.tsx", "places_.$id.tsx", "api.screens.v1.play.ts",
            "api.marketplace-screen.ts", "internal.pair.tsx", "profile.$id.tsx",
        ]), ["/api/marketplace-screen", "/api/screens", "/internal", "/places", "/profile"])

    def test_sites_cannot_claim_internal_api_paths(self):
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1"})
        for name in ["metrics.ts", "content.ts", "ws.ts"]:
            table = {"sites": [name], "internal": ["/metrics", "/internal/pulse"]}
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "reserved"):
                gateway.render(table, cfg, "/mime.types")

    def test_worlds_host_deploys_get_the_deploy_body_cap(self):
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1"})
        table = {"sites": [], "internal": ["/metrics"], "node": [], "aliases": [],
                 "deploy": ["/entities", "/content/entities"]}
        text = gateway.render(table, cfg, "/mime.types")
        deploy = re.search(r'location ~ "([^"]+)" \{\n[^}]*client_max_body_size 200m;', text)[1]
        for path in ["/entities", "/content/entities"]:
            with self.subTest(path=path):
                self.assertTrue(re.search(deploy, path))
        self.assertIsNone(re.search(deploy, "/worlds-content-server/entities"))
        worlds = re.search(r'location ~ "([^"]+)" \{\n[^}]*client_max_body_size 360m; '
                           r'client_body_timeout 300s; limit_req zone=deploy', text)[1]
        for path in ["/worlds-content-server/entities", "/worlds-content-server/entities/"]:
            with self.subTest(path=path):
                self.assertTrue(re.search(worlds, path))
        for path in ["/worlds-content-server/entities/active", "/entities"]:
            with self.subTest(path=path):
                self.assertIsNone(re.search(worlds, path))

    def test_worlds_caps_match_the_service_wire_limits(self):
        worlds = Path(__file__).parents[3] / "services/catalyst/worlds/src/core"

        def mib(path, name):
            source = (worlds / path).read_text()
            expr = re.search(rf"const {name}: usize = ([^;]+);", source)[1]
            for ref, value in re.findall(r"const (\w+): usize = ([^;]+);", source):
                expr = expr.replace(ref, f"({value})")
            self.assertRegex(expr, r"^[\d\s()*+]+$")
            total = eval(expr)
            self.assertEqual(total % (1024 * 1024), 0, name)
            return f"{total // (1024 * 1024)}m"

        self.assertEqual(gateway.WORLDS_DEPLOY_CAP, mib("deploy.rs", "MAX_UPLOAD_WIRE_SIZE_BYTES"))
        self.assertEqual(gateway.WORLDS_SETTINGS_CAP, mib("settings.rs", "MAX_SETTINGS_UPLOAD_WIRE_BYTES"))

    def worlds_locations(self):
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1", "GATEWAY_DOMAIN": "example.test"})
        table = {"sites": [], "internal": ["/metrics"], "node": [], "aliases": [],
                 "deploy": ["/entities", "/content/entities"]}
        text = gateway.render(table, cfg, "/mime.types")
        return text, [(re.compile(m[1].strip('"')), m[2]) for m in
                      re.finditer(r'location ~ ("[^"]+"|\S+) \{\n(.*?)\n\}', text, re.S)]

    def test_worlds_settings_get_the_service_settings_cap(self):
        _, regexes = self.worlds_locations()
        for path in ["/world/w.dcl.eth/settings", "/worlds-content-server/world/w.dcl.eth/settings",
                     "/worlds-content-server/world/w.dcl.eth/settings/"]:
            with self.subTest(path=path):
                body = next(body for regex, body in regexes if regex.search(path))
                self.assertIn("client_max_body_size 12m;", body)
        for path in ["/world/w.dcl.eth/settings/x", "/x/world/w.dcl.eth/settings", "/world/settings"]:
            with self.subTest(path=path):
                self.assertFalse(any("12m" in body for regex, body in regexes if regex.search(path)))

    def test_worlds_content_reads_use_the_content_rate_zone(self):
        _, regexes = self.worlds_locations()
        for path in ["/worlds-content-server/contents/bafy", "/worlds-content-server/ipfs/bafy"]:
            with self.subTest(path=path):
                body = next(body for regex, body in regexes if regex.search(path))
                self.assertIn("limit_req zone=content burst=1000 nodelay;", body)
        for path in ["/worlds-content-server/status", "/worlds-content-server/contentsx/a"]:
            with self.subTest(path=path):
                self.assertFalse(any("zone=content" in body for regex, body in regexes if regex.search(path)))

    def test_social_cards_keep_exact_backend_routes_without_shadowing_discovery_pages(self):
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1"})
        table = {"sites": ["places.tsx", "places_.place.$position.tsx", "places_.world.$name.tsx"],
                 "internal": [], "deploy": ["/entities"], "aliases": [],
                 "node": [{"path": "/places/api"}, {"path": "/places/place", "exact": True},
                          {"path": "/places/world", "exact": True}]}
        config = gateway.render(table, cfg, "/mime.types")
        for path in ["/places/place", "/places/world"]:
            self.assertIn(f"location = {path} {{\nproxy_pass http://apis;", config)
            self.assertIn(f"location = {path}/ {{\nproxy_pass http://apis;", config)
            self.assertNotIn(f"location ^~ {path}/", config)
        self.assertIn("location ^~ /places/api/ {\nproxy_pass http://apis;", config)
        self.assertIn("location /places/ {\nproxy_pass http://sites;", config)

    def test_bundled_event_image_uses_api_without_claiming_other_event_assets(self):
        path = "/events/assets/event-default.webp"
        routes = (Path(__file__).parents[1] / "routes.nix").read_text()
        self.assertRegex(routes, r'path = "/events/assets/event-default\.webp";\s+probe = "/events/assets/event-default\.webp";\s+exact = true;')
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1"})
        table = {"sites": ["events.tsx"], "internal": [], "deploy": [], "aliases": [],
                 "node": [{"path": path, "exact": True}]}
        config = gateway.render(table, cfg, "/mime.types")
        self.assertIn(f"location = {path} {{\nproxy_pass http://apis;", config)
        self.assertNotIn("location ^~ /events/assets/", config)
        self.assertIn("location /events/ {\nproxy_pass http://sites;", config)
