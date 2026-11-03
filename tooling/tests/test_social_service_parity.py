import copy
import importlib.util
import json
import re
import shutil
import tempfile
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("social_service_parity", ROOT / "tooling/social-service-parity.py")
PARITY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PARITY)


class SocialServiceParityInventoryTest(unittest.TestCase):
    def setUp(self):
        self.data = json.loads(PARITY.MANIFEST.read_text())

    def test_committed_inventory_and_readable_backlog_are_consistent(self):
        PARITY.validate(self.data)
        self.assertEqual(PARITY.report(self.data), PARITY.MANIFEST.with_name("PARITY.md").read_text())

    def test_losing_any_operation_is_rejected_including_streams_and_auxiliary_routes(self):
        for kind in self.data["baseline"]["counts"]:
            data = copy.deepcopy(self.data)
            removed = next(r for r in data["baseline"]["registrations"] if r["kind"] == kind)
            data["operations"] = [r for r in data["operations"] if r["id"] != removed["id"]]
            with self.subTest(kind=kind), self.assertRaisesRegex(AssertionError, "missing/extra"):
                PARITY.validate(data)

    def test_a_review_cannot_claim_a_missing_regression_test(self):
        row = next(r for r in self.data["operations"] if any(e.get("test") for e in r["evidence"]))
        next(e for e in row["evidence"] if e.get("test"))["test"] = "a_nonexistent_parity_test"
        with self.assertRaisesRegex(AssertionError, "missing test"):
            PARITY.validate(self.data)

    def test_a_known_gap_cannot_disappear_into_an_empty_note(self):
        self.data["operations"][0].update(status="known_gap", gaps=[], evidence=[{
            "path": "tooling/social-service-parity.py", "scope": "test-only evidence"
        }])
        with self.assertRaisesRegex(AssertionError, "known gap must name"):
            PARITY.validate(self.data)

    def test_a_field_change_with_unchanged_rpc_names_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for schema in self.data["baseline"]["protocol_schemas"]:
                target = root / schema["local_path"]
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / schema["local_path"], target)
            PARITY.validate_protocol_schemas(self.data["baseline"], root)
            path = root / PARITY.PROTO
            original = path.read_text()
            changed = original.replace("string address = 1;", "string address = 2;", 1)
            self.assertNotEqual(original, changed)
            self.assertEqual(re.findall(r"\brpc\s+(\w+)\s*\(", original),
                             re.findall(r"\brpc\s+(\w+)\s*\(", changed))
            path.write_text(changed)
            with self.assertRaisesRegex(AssertionError, "protocol schema hash differs"):
                PARITY.validate_protocol_schemas(self.data["baseline"], root)

    def test_editing_the_manifest_digest_cannot_repin_a_schema(self):
        self.data["baseline"]["protocol_schemas"][0]["sha256"] = "0" * 64
        with self.assertRaisesRegex(AssertionError, "protocol schema source pins differ"):
            PARITY.validate_protocol_schemas(self.data["baseline"])

    def test_a_short_route_prefix_does_not_reuse_the_longer_route_source_line(self):
        content = "router.delete('/v1/communities/:id/members/:address', handler)\nrouter.delete('/v1/communities/:id', handler)\n"
        matches = list(re.finditer(r"router\.(get|post|put|patch|delete)\(\s*['\"]([^'\"]+)", content))
        self.assertEqual([PARITY.line_at(content, m.start()) for m in matches], [1, 2])
