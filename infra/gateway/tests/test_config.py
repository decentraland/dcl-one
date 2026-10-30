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
        for path in ["/entities", "/content/entities", "/worlds-content-server/entities"]:
            with self.subTest(path=path):
                self.assertTrue(re.search(deploy, path))
        self.assertIsNone(re.search(deploy, "/worlds-content-server/entities/active"))

    def test_social_cards_keep_exact_backend_routes_without_shadowing_discovery_pages(self):
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1"})
        table = {"sites": ["places.tsx", "places_.place.$position.tsx", "places_.world.$name.tsx"],
                 "internal": [], "deploy": ["/entities"], "aliases": [],
                 "node": [{"path": "/places/api"}, {"path": "/places/place", "exact": True},
                          {"path": "/places/world", "exact": True}]}
        config = gateway.render(table, cfg, "/mime.types")
        for path in ["/places/place", "/places/world"]:
            self.assertIn(f"location = {path} {{\nproxy_pass http://apis;", config)
            self.assertNotIn(f"location ^~ {path}/", config)
        self.assertIn("location ^~ /places/api/ {\nproxy_pass http://apis;", config)
        self.assertIn("location /places/ {\nproxy_pass http://sites;", config)
