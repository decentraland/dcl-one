"""Verify the checked-in Stats compiler and license against their package pin."""
import hashlib
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
VENDOR = ROOT / "services/catalyst/src/metrics_matcher"


class StatsMatcherProvenance(unittest.TestCase):
    def test_distribution_members_match_the_pin(self):
        pin = json.loads((VENDOR / "upstream.lock.json").read_text(encoding="utf-8"))
        for member, local in {
            "package/dist/index.js": "path-to-regexp.cjs",
            "package/LICENSE": "path-to-regexp.LICENSE",
        }.items():
            with self.subTest(member=member):
                digest = hashlib.sha256((VENDOR / local).read_bytes()).hexdigest()
                self.assertEqual(digest, pin["members"][member])


if __name__ == "__main__":
    unittest.main()
