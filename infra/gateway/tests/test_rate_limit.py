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


def render(**env):
    cfg = gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1", "GATEWAY_DOMAIN": "example.test", **env})
    return gateway.render(TABLE, cfg, "/mime.types")


def location(text, key):
    match = re.search(r"\nlocation " + re.escape(key) + r" \{\n(.*?)\n\}", text, re.S)
    return match and match[1]


class StorageRateLimitTest(unittest.TestCase):
    def setUp(self):
        self.text = render()

    def test_storage_has_its_own_zone(self):
        body = location(self.text, "^~ /world-storage/")
        self.assertIsNotNone(body)
        self.assertIn("proxy_pass http://apis;", body)
        self.assertIn("limit_req zone=storage burst=400 nodelay;", body)
        self.assertNotIn("zone=read", body)
        self.assertIn("limit_req_zone $storage_limit_key zone=storage:10m rate=100r/s;", self.text)

    def test_every_storage_host_form_reaches_the_storage_location(self):
        for host, uri in [("example.test", "/world-storage/values/k"),
                          ("world-storage.example.test", "/values/k"),
                          ("world-storage.example.test", "/world-storage/values/k"),
                          ("gateway.example.test", "/world-storage/values/k")]:
            with self.subTest(host=host, uri=uri):
                self.assertEqual(nginx_map.route(self.text, host, uri)[0], "/world-storage/values/k")

    def test_anonymous_clients_are_limited_per_ip(self):
        key = nginx_map.Request(nginx_map.parse(self.text), storage_trusted="0", binary_remote_addr="198.51.100.7")
        self.assertEqual(key["storage_limit_key"], "198.51.100.7")

    def test_bare_storage_path_uses_the_same_budget(self):
        body = location(self.text, "= /world-storage")
        self.assertIsNotNone(body)
        self.assertIn("limit_req zone=storage burst=400 nodelay;", body)

    def test_writes_keep_the_lower_budget(self):
        for trusted in ["0", "1"]:
            for method in ["GET", "HEAD", "OPTIONS", "PUT", "DELETE", "POST"]:
                key = nginx_map.Request(nginx_map.parse(self.text), storage_trusted=trusted,
                                        request_method=method, binary_remote_addr="198.51.100.7")
                expected = "198.51.100.7" if trusted == "0" and method in ["PUT", "DELETE", "POST"] else ""
                self.assertEqual(key["storage_write_key"], expected, (trusted, method))
        self.assertIn("zone=storage_write:10m rate=30r/s", self.text)


class StorageTrustedCidrsTest(unittest.TestCase):
    def setUp(self):
        self.text = render(GATEWAY_STORAGE_TRUSTED_CIDRS="10.1.2.3/8, 2001:db8::1/64")

    def test_trusted_networks_are_canonicalized_into_the_geo(self):
        self.assertIn("geo $storage_trusted { default 0; 10.0.0.0/8 1; 2001:db8::/64 1; }", self.text)

    def test_trusted_sources_use_a_bounded_separate_zone(self):
        key = nginx_map.Request(nginx_map.parse(self.text), storage_trusted="1", binary_remote_addr="10.1.2.3")
        self.assertEqual(key["storage_limit_key"], "")
        self.assertEqual(key["storage_trusted_key"], "10.1.2.3")
        self.assertIn("zone=storage_trusted:10m rate=1000r/s", self.text)
        self.assertIn("limit_req zone=storage_trusted burst=1000 nodelay", self.text)

    def test_rejects_invalid_trusted_networks(self):
        for value in ["10.0.0.0/8;", "allow all", "example.org", "203.0.113.1/33"]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1", "GATEWAY_STORAGE_TRUSTED_CIDRS": value})

    def test_storage_trust_is_closed_by_default(self):
        self.assertEqual(gateway.settings({"GATEWAY_RESOLVERS": "127.0.0.1"})["storage_trusted"], [])


if __name__ == "__main__":
    unittest.main()
