"""Catalyst's entry points link the nix shell's V8 (flake.nix) and leave other builds alone."""
import importlib.util
import os
from pathlib import Path
import re
import runpy
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).parents[2]
ARCHIVE = "/nix/store/example-librusty_v8.a"


class CatalystV8ArchiveTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        # A `cargo` that reports the archive it was given instead of building.
        self.bin = Path(temporary.name)
        cargo = self.bin / "cargo"
        cargo.write_text('#!/usr/bin/env bash\necho "archive=${RUSTY_V8_ARCHIVE-unset}"\n')
        cargo.chmod(0o755)

    def environment(self, archive):
        env = {key: value for key, value in os.environ.items()
               if key not in ("RUSTY_V8_ARCHIVE", "CATALYST_RUSTY_V8_ARCHIVE", "MAKEFLAGS", "MFLAGS")}
        env["PATH"] = f"{self.bin}{os.pathsep}{env['PATH']}"
        if archive is not None:
            env["CATALYST_RUSTY_V8_ARCHIVE"] = archive
        return env

    def run_with(self, command, archive, cwd=ROOT):
        return subprocess.run(command, cwd=cwd, env=self.environment(archive), text=True,
                              stdout=subprocess.PIPE, check=True).stdout

    def test_smoke_cache_build_links_the_nix_v8(self):
        build_env = runpy.run_path(str(ROOT / "tooling/catalyst-smoke-cache.py"))["build_env"]
        self.assertEqual(build_env({"CATALYST_RUSTY_V8_ARCHIVE": ARCHIVE})["RUSTY_V8_ARCHIVE"], ARCHIVE)
        self.assertNotIn("RUSTY_V8_ARCHIVE", build_env({}))
        self.assertNotIn("RUSTY_V8_ARCHIVE", build_env({"CATALYST_RUSTY_V8_ARCHIVE": ""}))

    def test_schema_capture_cargo_links_the_nix_v8(self):
        source = (ROOT / "tooling/capture-social-schema-v2.cjs").read_text()
        snippet = re.search(r"^const CATALYST_CARGO = '(.+)'$", source, re.M).group(1)
        self.assertIn("'bash', '-c', CATALYST_CARGO, 'bash', 'test'", source)
        command = ["bash", "-c", snippet, "bash", "test"]
        self.assertEqual(self.run_with(command, ARCHIVE).strip(), f"archive={ARCHIVE}")
        self.assertEqual(self.run_with(command, None).strip(), "archive=unset")

    def test_dev_test_script_links_the_nix_v8(self):
        line = next(line for line in (ROOT / "tooling/dev-test.sh").read_text().splitlines()
                    if "RUSTY_V8_ARCHIVE=" in line)
        command = ["bash", "-c", f"set -euo pipefail; {line}; cargo"]
        self.assertEqual(self.run_with(command, ARCHIVE).strip(), f"archive={ARCHIVE}")
        self.assertEqual(self.run_with(command, None).strip(), "archive=unset")

    def test_makefile_catalyst_targets_link_the_nix_v8(self):
        command = ["make", "--no-print-directory", "catalyst-build"]
        self.assertTrue(self.run_with(command, ARCHIVE).strip().endswith(f"archive={ARCHIVE}"))
        self.assertTrue(self.run_with(command, None).strip().endswith("archive=unset"))

    def test_foundation_dev_api_build_links_the_nix_v8(self):
        spec = importlib.util.spec_from_file_location("foundation_dev_v8", ROOT / "tooling/dev.py")
        dev = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(dev)
        for archive, expected in ((ARCHIVE, ARCHIVE), (None, None)):
            calls = []

            def stop(argv, **kwargs):
                calls.append(kwargs.get("env", {}).get("RUSTY_V8_ARCHIVE"))
                raise SystemExit

            with mock.patch.dict(os.environ, self.environment(archive), clear=True), \
                    mock.patch.object(dev, "settings", lambda: {}), \
                    mock.patch.object(dev, "run", stop), self.assertRaises(SystemExit):
                dev.task("api-build")
            self.assertEqual(calls, [expected])


if __name__ == "__main__":
    unittest.main()
