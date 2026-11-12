"""Exercise command routing and fail-closed sampling without database credentials."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class SoakTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        root = Path(self.directory.name)
        self.log = root / "calls"
        binary = root / "worker"
        binary.write_text('#!/usr/bin/env bash\nprintf "%s\\n" "$*" >> "$SOAK_TEST_LOG"\n'
                          'if [[ "$1" == "${SOAK_TEST_FAIL:-}" ]]; then exit 1; fi\n')
        binary.chmod(0o700)
        self.env = dict(os.environ, MIGRATION_SYNC_BIN=str(binary), SOAK_TEST_LOG=str(self.log))

    def run_soak(self, *arguments):
        return subprocess.run(
            ["bash", str(Path(__file__).with_name("soak.sh")), *arguments],
            env=self.env, capture_output=True, text=True, timeout=5
        )

    def test_routes_each_source_and_preserves_places_default(self):
        for source in (None, "places", "events", "social-settings"):
            with self.subTest(source=source):
                self.log.write_text("")
                args = ["rehearsal", "1", "1"] + ([] if source is None else [source])
                result = self.run_soak(*args)
                self.assertEqual(result.returncode, 0, result.stderr)
                reconcile = "reconcile" if source in (None, "places") else f"reconcile-domain --source {source}"
                self.assertEqual(self.log.read_text().splitlines(), [
                    "status --stream rehearsal", "metrics --stream rehearsal",
                    "check --stream rehearsal", f"{reconcile} --stream rehearsal"
                ])

    def test_invalid_input_never_contacts_worker(self):
        for args in (("bad.stream",), ("valid", "0"), ("valid", "1", "1", "shop"),
                     ("valid", "1", "1", "places", "extra"),
                     ("valid", "9223372036854775808"), ("valid", "1000001"),
                     ("valid", "1", "86401"), ("valid", "1", "99999999999999999999")):
            with self.subTest(args=args):
                self.assertNotEqual(self.run_soak(*args).returncode, 0)
                self.assertFalse(self.log.exists())

    def test_failed_health_or_reconciliation_stops_sampling(self):
        for command in ("check", "reconcile-domain"):
            with self.subTest(command=command):
                self.log.write_text("")
                self.env["SOAK_TEST_FAIL"] = command
                self.assertNotEqual(self.run_soak("rehearsal", "2", "1", "events").returncode, 0)
                calls = self.log.read_text().splitlines()
                self.assertEqual(calls.count("status --stream rehearsal"), 1)
                self.assertTrue(calls[-1].startswith(command + " "))


if __name__ == "__main__":
    unittest.main()
