import importlib.util
from pathlib import Path
import re
import unittest

import nginx_map

SPEC = importlib.util.spec_from_file_location("gateway", Path(__file__).parents[1] / "gateway.py")
gateway = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gateway)

TABLE = {"sites": [], "internal": ["/internal/pulse", "/metrics"], "node": [], "aliases": [],
         "deploy": ["/entities", "/content/entities"]}


def route(host, request_uri):
    cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1", "GATEWAY_DOMAIN": "example.test"})
    return nginx_map.route(gateway.render(TABLE, cfg, "/mime.types"), host, request_uri)


class HostAliasTest(unittest.TestCase):
    def test_existing_aliases_keep_their_routes(self):
        self.assertEqual(route("abcdn.example.test", "/x"), ("/ab-cdn/x", "/x"))
        self.assertEqual(route("peer.example.test", "/about"), ("/about", "/about"))
        self.assertEqual(route("places.example.test", "/api/x"), ("/places/api/x", "/api/x"))
        self.assertEqual(route("gateway.example.test", "/peer/about?x=1"), ("/about", "/about?x=1"))
        self.assertEqual(route("world-storage.example.test", "/values?limit=10"),
                         ("/world-storage/values", "/values?limit=10"))

    def test_storage_subdomain_routes_to_world_storage(self):
        self.assertEqual(route("storage.example.test", "/values/k"), ("/world-storage/values/k", "/values/k"))

    def test_storage_gateway_prefix_routes_to_world_storage(self):
        self.assertEqual(route("gateway.example.test", "/storage/values/k"), ("/world-storage/values/k", "/values/k"))

    def test_storage_subdomain_does_not_double_prefix(self):
        self.assertEqual(route("storage.example.test", "/world-storage/values/k"),
                         ("/world-storage/values/k", "/world-storage/values/k"))

    def test_preview_gatekeeper_routes_to_comms_gatekeeper(self):
        self.assertEqual(route("comms-gatekeeper-local.example.test", "/get-scene-adapter"),
                         ("/comms-gatekeeper/get-scene-adapter", "/get-scene-adapter"))
        self.assertEqual(route("gateway.example.test", "/comms-gatekeeper-local/get-scene-adapter"),
                         ("/comms-gatekeeper/get-scene-adapter", "/get-scene-adapter"))

    def test_badges_host_carries_upstreams_whole_surface(self):
        # <node>/badges is the badges base: upstream's own /badges/... paths
        # are prefixed on this host too. The node root aliases only the
        # wallet reads (root-aliases.nix).
        for path in ["/badges", "/badges/traveler", "/badges/traveler/tiers",
                     "/badges/halloweeks_2026/backfill", "/categories", "/status",
                     "/users/0x1234567890abcdef1234567890abcdef12345678/preview"]:
            with self.subTest(path=path):
                self.assertEqual(route("badges.example.test", path), ("/badges" + path, path))
                self.assertEqual(route("gateway.example.test", "/badges" + path), ("/badges" + path, path))

    def test_badges_host_keeps_already_prefixed_paths(self):
        # A second segment the badges router mounts is taken as the /badges
        # mount, never as a badge id.
        for path in ["/badges/badges", "/badges/badges/traveler/tiers", "/badges/categories",
                     "/badges/status", "/badges/assets/traveler/2d.png",
                     "/badges/users/0x1234567890abcdef1234567890abcdef12345678/badges"]:
            with self.subTest(path=path):
                self.assertEqual(route("badges.example.test", path), (path, path))

    def test_badges_mounts_match_the_badges_router(self):
        router = (Path(__file__).parents[3] / "services/catalyst/social/src/badges/mod.rs").read_text()
        mounts = set(re.findall(r'\.route\(\s*"/([^/"]+)', router))
        self.assertEqual(mounts, set(gateway.BADGES_MOUNTS))

    def test_credits_host_serves_credits_servers_own_paths(self):
        # credits-server's /credits/... paths are served at the same subpaths
        # of the node's /credits mount: the pack list, the pack purchase (not
        # the merchandise checkout at the mount's /checkout), its orders and
        # the credit authorizations, as the rest of credits-server's surface.
        for path in ["/credits", "/credits/packs", "/credits/checkout", "/credits/orders/ord_1",
                     "/credits/orders/ord_1/cancel-return", "/credits/orders/ord_1/resume",
                     "/credits/authorize", "/credits/authorize/batch",
                     "/credits/authorize-publication", "/credits/iap/quote", "/users/0xab/credits",
                     "/seasons"]:
            with self.subTest(path=path):
                self.assertEqual(route("credits.example.test", path), ("/credits" + path, path))

    def test_credits_host_keeps_other_prefixed_paths(self):
        # Stripe's webhook for credits-server is /credits/webhook, the node's
        # mount's /webhook; a path already written for the node passes as sent.
        # The node's own /packs/... and /checkout/{id} keep their prefixed form:
        # credits-server's pack list and pack purchase are the bare paths alone.
        for path in ["/credits/webhook", "/credits/credits/packs", "/credits/users/0xab/credits",
                     "/credits/checkout-other", "/credits/packs/sku_1/intent",
                     "/credits/packs/sku_1/mock-purchase", "/credits/checkout/chk_1"]:
            with self.subTest(path=path):
                self.assertEqual(route("credits.example.test", path), (path, path))

    def test_credits_server_mounts_cover_the_credits_router(self):
        # Every /credits/<x> route the node's credits mount serves, and every
        # /credits/credits/<x> path the Shop's path table lists, is one of
        # credits-server's own paths, so the credits host reaches it; one with
        # subpaths is a mount, which takes any tail. No mount shares its first
        # segment with a route of the node's own at the mount's root, whose
        # prefixed form would be prefixed twice.
        shop = Path(__file__).parents[3] / "services/catalyst/shop/src"
        router = (shop / "credits/mod.rs").read_text()
        table = (shop / "paths.rs").read_text()
        paths = set(re.findall(r'\.route\(\s*"/credits/([^/"{]+)', router))
        paths |= set(re.findall(r'/credits/credits/([a-z-]+)', table))
        self.assertTrue(paths, "the credits router's /credits/... routes")
        self.assertLessEqual(paths, set(gateway.CREDITS_SERVER_PATHS) | set(gateway.CREDITS_SERVER_MOUNTS))
        deep = set(re.findall(r'\.route\(\s*"/credits/([^/"{]+)/', router))
        deep |= set(re.findall(r'/credits/credits/([a-z-]+)/', table))
        self.assertTrue(deep, "the credits router's /credits/<x>/... routes")
        self.assertLessEqual(deep, set(gateway.CREDITS_SERVER_MOUNTS))
        # The node's own routes include those of the routers the mount merges
        # (the operator routes among them).
        merged = re.findall(r'\.merge\(handlers::(\w+)::router\(\)\)', router)
        self.assertIn("admin", merged, "the routers the credits mount merges")
        sources = [router] + [(shop / f"credits/handlers/{name}.rs").read_text() for name in merged]
        own = set().union(*(re.findall(r'\.route\(\s*"/([^/"{]+)/', src) for src in sources)) - {"credits"}
        self.assertTrue(own, "the credits router's own /<x>/... routes")
        self.assertIn("admin", own)
        self.assertTrue(own.isdisjoint(gateway.CREDITS_SERVER_MOUNTS), own & set(gateway.CREDITS_SERVER_MOUNTS))

    def test_worlds_host_reaches_the_prefixed_worlds_routes(self):
        for path in ["/world/w.dcl.eth/settings", "/livekit-webhook", "/health/live",
                     "/contents/bafy", "/entities"]:
            with self.subTest(path=path):
                self.assertEqual(route("worlds-content-server.example.test", path),
                                 ("/worlds-content-server" + path, path))

    def test_realm_hosts_pass_the_services_a_scene_builds_from_its_realm_base(self):
        # A scene asks `<realm base URL>/<service>/...`; a realm base is one of these hosts.
        for host in ["worlds-content-server", "realm-provider", "realm-provider-ea"]:
            for path in ["/comms-gatekeeper/scene-admin", "/cast-presenter-service/presentations?x=1"]:
                with self.subTest(host=host, path=path):
                    self.assertEqual(route(host + ".example.test", path), (path.split("?")[0], path))
        # The host's own paths and every other host keep the prefix.
        for host, path in [("worlds-content-server", "/comms-gatekeeper"),
                           ("worlds-content-server", "/comms-gatekeeperx/a"),
                           ("realm-provider-ea", "/main/comms-gatekeeper/a"),
                           ("places", "/comms-gatekeeper/scene-admin")]:
            with self.subTest(host=host, path=path):
                self.assertEqual(route(host + ".example.test", path), (f"/{host}{path}", path))


    def test_auth_relay_owns_cors_on_both_mounts(self):
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1", "GATEWAY_DOMAIN": "example.test"})
        config = gateway.render(TABLE, cfg, "/mime.types")
        maps = nginx_map.parse(config)
        for host, path, expected in [
            ("example.test", "/auth-api/x", ""),
            ("example.test", "/auth-api", ""),
            ("auth-api.example.test", "/x", ""),
            ("example.test", "/places/x", "*"),
        ]:
            with self.subTest(host=host, path=path):
                uri, _ = nginx_map.route(config, host, path)
                request = nginx_map.Request(maps, uri=uri, upstream_http_access_control_allow_origin="")
                self.assertEqual(request["cors_fallback"], expected)

    def test_auth_relay_socket_uses_the_affinity_upstream_for_both_mounts(self):
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1", "GATEWAY_DOMAIN": "example.test"})
        config = gateway.render(TABLE, cfg, "/mime.types")
        prefixes = dict(re.findall(r"^location (?:\^~ )?(/\S*) \{\nproxy_pass http://(\w+);", config, re.M))
        self.assertIn("location ^~ /auth-api/socket.io/ {", config)
        for host, path in [("example.test", "/auth-api/socket.io/"),
                           ("auth-api.example.test", "/socket.io/")]:
            with self.subTest(host=host, path=path):
                uri, _ = nginx_map.route(config, host, path + "?EIO=4&transport=polling")
                match = max((prefix for prefix in prefixes if uri.startswith(prefix)), key=len)
                self.assertEqual(match, "/auth-api/socket.io/")
                self.assertEqual(prefixes[match], "auth_socket")


class AccountHostTest(unittest.TestCase):
    def test_account_host_reaches_the_sites_confirmation_pages(self):
        root = Path(__file__).parents[3]
        sites = sorted(path.name for path in (root / "apps/sites/src/routes/routes").iterdir())
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1", "GATEWAY_DOMAIN": "example.test"})
        config = gateway.render({**TABLE, "sites": sites}, cfg, "/mime.types")
        uri = "/confirm-email-challenge/abc?address=0x1&source=account"
        self.assertEqual(nginx_map.route(config, "account.example.test", uri),
                         ("/account/confirm-email-challenge/abc", uri))
        self.assertIn("location /account/ {\nproxy_pass http://sites;", config)
        # The confirm request itself stays the node's on the apex host.
        self.assertEqual(nginx_map.route(config, "example.test", "/notifications/confirm-email")[0],
                         "/notifications/confirm-email")


class CorsFallbackTest(unittest.TestCase):
    """An answer without an allow-origin gets `*` from the gateway, except the builder's,
    whose allow-list (`CORS_ORIGIN`) withholds it from an unlisted origin on purpose."""

    def fallback(self, host, request_uri, upstream=""):
        cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1", "GATEWAY_DOMAIN": "example.test"})
        text = gateway.render(TABLE, cfg, "/mime.types")
        uri, _ = nginx_map.route(text, host, request_uri)
        request = nginx_map.Request(nginx_map.parse(text), uri=uri,
                                    upstream_http_access_control_allow_origin=upstream)
        return request["cors_fallback"]

    @staticmethod
    def builder_root_routes():
        """Every route of the builder's root mount (`api_routes` in builder/src/lib.rs), with sample parameters,
        except `/v1/items...`, which the apex host gives to another area's alias."""
        source = (Path(__file__).parents[3] / "services/catalyst/builder/src/lib.rs").read_text()
        table = source[source.index("fn api_routes()"):source.index("fn finish(")]
        routes = {re.sub(r"\{[^}]+\}", "x", path.replace("{address}", "0x" + "Ab" * 20))
                  for path in re.findall(r'"(/v1/[^"]*)"', table)}
        return sorted(route for route in routes if not re.match(r"/v1/items(/|$)", route))

    def test_the_builder_answers_keep_the_origins_the_node_decided(self):
        routes = self.builder_root_routes()
        self.assertIn("/v1/thirdParties/x/slots", routes)
        for host, path in [("builder-api.example.test", "/v1/collections/x"),
                           ("example.test", "/builder-api/v1/storage/contents/x"),
                           *(("example.test", route) for route in routes)]:
            with self.subTest(host=host, path=path):
                self.assertEqual(self.fallback(host, path), "")
                self.assertEqual(self.fallback(host, path, "https://create.example.test"), "")

    def test_every_other_answer_without_an_origin_gets_the_wildcard(self):
        for host, path in [("example.test", "/shop-api/v1/x"), ("example.test", "/v1/communities/x"),
                           ("example.test", "/v1/items/x"), ("example.test", "/v1/collectionsx"),
                           ("example.test", "/v1/cart/items"),
                           ("places.example.test", "/api/places"), ("example.test", "/builder-apix/v1")]:
            with self.subTest(host=host, path=path):
                self.assertEqual(self.fallback(host, path), "*")
                self.assertEqual(self.fallback(host, path, "https://a.example.test"), "")
