"""Compare the shard planner with Cargo's test executables."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class TargetPlanTests(unittest.TestCase):
    def test_shard_inventory_matches_cargo_with_a_feature_gated_integration_test(self):
        planner = [sys.executable, str(ROOT / "tooling/catalyst-test-targets.py")]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "Cargo.toml").write_text(
                '[package]\nname="sample"\nversion="0.1.0"\nedition="2024"\n'
                '[features]\nextra=[]\n[[test]]\nname="optional"\npath="tests/optional.rs"\n'
                'required-features=["extra"]\n'
            )
            for filename in ("src/lib.rs", "tests/ordinary.rs", "tests/optional.rs"):
                path = directory / filename
                path.parent.mkdir(exist_ok=True)
                path.write_text("#[test] fn present() {}\n")
            subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=directory, check=True)
            for features in ([], ["--features", "extra"]):
                arguments = ["--workspace", *features]
                planned = subprocess.check_output([*planner, "plan", *arguments], cwd=directory, text=True)
                built = subprocess.check_output(
                    ["cargo", "test", "--locked", "--offline", *arguments, "--no-run", "--message-format=json"],
                    cwd=directory, text=True,
                )
                artifacts = []
                for line in built.splitlines():
                    unit = json.loads(line)
                    if unit.get("executable") and unit["profile"]["test"]:
                        target = unit["target"]
                        source = Path(target["src_path"]).relative_to(directory.resolve())
                        artifacts.append((str(source), target["kind"][0], unit["executable"]))
                inventory = [row.split("\t") for row in planned.splitlines()]
                self.assertCountEqual([(src, kind) for src, kind, _ in inventory],
                                      [(src, kind) for src, kind, _ in artifacts])
                (directory / "inventory").write_text(planned)
                (directory / "artifacts").write_text("".join("\t".join(row) + "\n" for row in artifacts))
                (directory / "shard").write_text("test/ordinary\n")
                resolved = subprocess.check_output(
                    [*planner, "resolve", "shard", "inventory", "artifacts"], cwd=directory, text=True,
                )
                self.assertEqual(resolved.strip(), next(path for src, _, path in artifacts if src == "tests/ordinary.rs"))

    def test_inventory_build_includes_mandatory_targets_from_other_shards(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tooling = root / "tooling"
            tooling.mkdir()
            for name in ("catalyst-test.sh", "catalyst-test-targets.py"):
                shutil.copy2(ROOT / "tooling" / name, tooling / name)
            (tooling / "catalyst-test-durations.tsv").touch()
            workspace = root / "services/catalyst"
            (workspace / "tests").mkdir(parents=True)
            (workspace / "src").mkdir()
            (workspace / "Cargo.toml").write_text('[package]\nname="sample"\nversion="0.1.0"\nedition="2024"\n')
            for source in ("src/lib.rs", "tests/first.rs", "tests/second.rs", "tests/third.rs"):
                (workspace / source).write_text('#[test] fn inventory_only() { panic!("test executed"); }')
            subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=workspace, check=True)
            inventory = root / "artifacts.jsonl"

            def compile_inventory(*extra):
                subprocess.run([tooling / "catalyst-test.sh", "--write-inventory", inventory, *extra, "--", "--workspace"],
                               env={**os.environ, "CATALYST_TEST_SHARD": "1/2", "CATALYST_TEST_DOC_SHARD": "1",
                                    "CARGO_TARGET_DIR": str(workspace / "target")}, check=True)
                return {unit["target"]["name"] for line in inventory.read_text().splitlines()
                        if (unit := json.loads(line)).get("executable") and unit["target"]["kind"] == ["test"]}

            assigned = compile_inventory()
            missing = {"first", "second", "third"} - assigned
            self.assertTrue(assigned and missing)
            extra = sorted(missing)[0]
            self.assertEqual(compile_inventory(extra), assigned | {extra})
