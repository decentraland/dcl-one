"""Reject corpus loss and unverified regeneration inputs before execution."""
import importlib.util
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("fixtures", ROOT / "tooling/archipelago-fixtures.py")
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class FixtureTests(unittest.TestCase):
    def test_checked_in_corpora_have_an_installed_integrity_guard(self):
        path = ROOT / "tooling/archipelago-fixtures.py"
        self.assertTrue(path.is_file(), "fixture integrity needs a standalone offline check")
        guard.check(ROOT)

    def test_selected_recovery_keeps_complete_cap_bytes_and_exact_identity_metadata(self):
        document = json.loads((ROOT / (guard.RECOVERY + "pulse_selected_recovery.json")).read_text())
        guard.check_selected_recovery(document)
        changes = [
            lambda data: data["intent_vectors"].pop(),
            lambda data: data["reply_vectors"].append(data["reply_vectors"][0]),
            lambda data: data["identity_boundary_vectors"][2].update(identity_decimal=9007199254740992),
            lambda data: data["encoded_reply_boundaries"][0].update(wire_hex="00"),
            lambda data: data["encoded_reply_boundaries"][0]["construction"].update(pending_count=0),
        ]
        for change in changes:
            changed = json.loads(json.dumps(document))
            change(changed)
            with self.subTest(change=change), self.assertRaises(ValueError):
                guard.check_selected_recovery(changed)

    def test_selected_recovery_content_mutations_fail_integrity(self):
        with tempfile.TemporaryDirectory(prefix="pulse-recovery-integrity-") as temporary:
            root = Path(temporary)
            self.copy_fixture(root)
            path = root / (guard.RECOVERY + "pulse_selected_recovery.json")
            original = path.read_text()
            changes = [
                lambda data: data["reply_vectors"][1]["fields"].update(selected_origin=0),
                lambda data: data["encoded_reply_boundaries"][0].update(wire_hex="00" * 65536),
                lambda data: data["encoded_reply_boundaries"][1]["construction"].update(displaced_cluster_repeat=65356),
            ]
            for change in changes:
                document = json.loads(original)
                change(document)
                path.write_text(json.dumps(document), encoding="utf-8")
                with self.subTest(change=change), self.assertRaisesRegex(ValueError, "integrity mismatch"):
                    guard.check(root)
            path.unlink()
            with self.assertRaises(FileNotFoundError):
                guard.check(root)

    def copy_fixture(self, root):
        for name in guard.PINS:
            destination = root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, destination)

    def test_lost_case_runtime_manifest_or_generator_fails_closed(self):
        with tempfile.TemporaryDirectory(prefix="archipelago-integrity-") as temporary:
            root = Path(temporary)
            self.copy_fixture(root)
            for name in guard.PINS:
                file = root / name
                original = file.read_bytes()
                file.write_bytes(original + b" ")
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "integrity mismatch"):
                    guard.check(root)
                file.write_bytes(original)
            corpus = root / (guard.WIRE + "legacy-fixtures.json")
            document = json.loads(corpus.read_text())
            document["cases"] = []
            corpus.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "integrity mismatch"):
                guard.check(root)

    def test_checkout_crlf_is_explicitly_normalized(self):
        with tempfile.TemporaryDirectory(prefix="archipelago-crlf-") as temporary:
            root = Path(temporary)
            self.copy_fixture(root)
            for name in guard.PINS:
                path = root / name
                path.write_bytes(guard.lf_bytes(path).replace(b"\n", b"\r\n"))
            guard.check(root)

    def test_unsafe_application_paths_are_rejected(self):
        for member in ["", "../escape", "/absolute", "C:/drive", "folder\\escape"]:
            with self.subTest(member=member), self.assertRaises(ValueError):
                guard.member_path(ROOT, member)

    def test_changed_runtime_is_rejected_without_execution(self):
        with tempfile.TemporaryDirectory(prefix="archipelago-runtime-") as temporary:
            node = Path(temporary) / "node.exe"
            node.write_bytes(b"same version string; modified runtime")
            with mock.patch.object(guard.sys, "platform", "win32"), mock.patch.object(guard.subprocess, "run") as execute, \
                 self.assertRaisesRegex(ValueError, "integrity mismatch"):
                guard.regenerate(ROOT, Path(temporary), node, temporary, Path(temporary) / "out")
            execute.assert_not_called()

    def test_changed_application_is_rejected_without_execution(self):
        with tempfile.TemporaryDirectory(prefix="archipelago-source-") as temporary:
            upstream = Path(temporary)
            lock = guard.check(ROOT)
            member = next(iter(lock["sources"]))
            file = guard.member_path(upstream, member)
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(b"modified source with advertised original revision")
            with mock.patch.object(guard.sys, "platform", "win32"), \
                 mock.patch.object(guard, "NODE_WINDOWS_X64_SHA256", hashlib.sha256(b"node").hexdigest()), \
                 mock.patch.object(guard.subprocess, "run") as execute:
                node = upstream / "node.exe"
                node.write_bytes(b"node")
                with self.assertRaisesRegex(ValueError, "integrity mismatch"):
                    guard.regenerate(ROOT, upstream, node, temporary, upstream / "out")
            execute.assert_not_called()

    def test_cli_requires_explicit_oracle_inputs(self):
        result = subprocess.run([sys.executable, "-B", str(ROOT / "tooling/archipelago-fixtures.py"), "regenerate"],
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("required", result.stderr)


if __name__ == "__main__":
    unittest.main()
