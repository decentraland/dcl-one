import importlib.util
from pathlib import Path
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
