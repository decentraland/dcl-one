"""CI inventory fixtures and a dependency-free Cargo build-context regression."""
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("regressions", ROOT / "tooling/dev-regressions.py")
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class InventoryTests(unittest.TestCase):
    def test_database_generator_tracks_root_across_shared_cargo_target(self):
        # Exercise Cargo's cached build-script dependency contract, not its source text.
        with tempfile.TemporaryDirectory(prefix="db-build-context-") as temporary:
            probe = Path(temporary)
            fixture = probe / "crate"
            (fixture / "src").mkdir(parents=True)
            shutil.copyfile(ROOT / "services/catalyst/db/build.rs", fixture / "build.rs")
            (fixture / "Cargo.toml").write_text(
                '[package]\nname="db-build-context-probe"\nversion="0.0.0"\nedition="2024"\n[workspace]\n',
                encoding="utf-8",
            )
            (fixture / "Cargo.lock").write_text(
                'version = 4\n\n[[package]]\nname = "db-build-context-probe"\nversion = "0.0.0"\n',
                encoding="utf-8",
            )
            (fixture / "src/main.rs").write_text(
                'fn main() { print!("{}\n-- PROBE MIGRATIONS --\n{}", '
                'include_str!(concat!(env!("OUT_DIR"), "/schema.sql")), '
                'include_str!(concat!(env!("OUT_DIR"), "/migrations.sql"))); }\n',
                encoding="utf-8",
            )
            roots = []
            for marker in ("A", "B"):
                source = probe / ("root-" + marker)
                migrations = source / "schemas/migrations"
                migrations.mkdir(parents=True)
                (source / "schemas/00_context.sql").write_text(
                    f"-- SCHEMA_CONTEXT_{marker}\nCREATE TABLE context_{marker.lower()}(id integer);\n",
                    encoding="utf-8",
                )
                (migrations / "0001_context.sql").write_text(
                    f"-- MIGRATION_CONTEXT_{marker}\nALTER TABLE context_{marker.lower()} ADD COLUMN label text;\n",
                    encoding="utf-8",
                )
                roots.append((marker, source))
            target = probe / "shared-target"
            executable = target / "debug" / ("db-build-context-probe.exe" if os.name == "nt" else "db-build-context-probe")
            previous = None
            generated_paths = None
            for marker, source in roots:
                result = subprocess.run(
                    ["cargo", "build", "--locked", "--offline", "--manifest-path",
                     str(fixture / "Cargo.toml"), "--target-dir", str(target)],
                    cwd=fixture,
                    env={**os.environ, "DCL_ONE_ROOT": str(source)},
                    capture_output=True, text=True, timeout=60,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                schemas = list((target / "debug/build").glob("db-build-context-probe-*/out/schema.sql"))
                self.assertEqual(len(schemas), 1)
                paths = (schemas[0], schemas[0].with_name("migrations.sql"))
                if generated_paths is not None:
                    self.assertEqual(paths, generated_paths, "one shared Cargo output directory is reused")
                generated_paths = paths
                generated = tuple(path.read_bytes() for path in paths)
                for kind, content in zip(("SCHEMA", "MIGRATION"), generated):
                    self.assertIn(f"{kind}_CONTEXT_{marker}".encode(), content)
                    other = "B" if marker == "A" else "A"
                    self.assertNotIn(f"{kind}_CONTEXT_{other}".encode(), content)
                embedded = subprocess.run([str(executable)], capture_output=True, timeout=10, check=True).stdout
                self.assertEqual(embedded, generated[0] + b"\n-- PROBE MIGRATIONS --\n" + generated[1])
                if previous is not None:
                    self.assertNotEqual(generated[0], previous[0])
                    self.assertNotEqual(generated[1], previous[1])
                previous = generated

    def test_good(self):
        guard.verify(["required"], "required: test\nother: test\n", "other: test\n", "fixture")

    def test_known_bad_inventories(self):
        for normal, ignored in [("", ""), ("renamed: test\n", ""),
                                ("required: test\n", "required: test\n"),
                                ("required: test\nrequired: test\n", ""),
                                ("required: test\nmalformed\n", "")]:
            with self.subTest(normal=normal, ignored=ignored), self.assertRaises(ValueError):
                guard.verify(["required"], normal, ignored, "bad fixture")
        for required in [[], ["required", "required"]]:
            with self.assertRaises(ValueError):
                guard.verify(required, "required: test\n", "", "bad manifest")

    def test_active_broad_invocation(self):
        guard.validate_invocation(guard.BROAD + "\n", {})
        for invocation in [guard.BROAD.replace("--workspace", "-p catalyst-comms"),
                           guard.BROAD + " --exclude catalyst", guard.BROAD + " case_name",
                           guard.BROAD + " -- --skip required", guard.BROAD + " -- --ignored",
                           guard.BROAD.replace("cargo test", "cargo test --test unrelated")]:
            with self.subTest(invocation=invocation), self.assertRaises(ValueError):
                guard.validate_invocation(invocation, {})
        with self.assertRaises(ValueError):
            guard.validate_invocation(guard.BROAD, {"MAKEFLAGS": "NODE_CRATES=-p catalyst-comms"})

    def test_existing_bevy_artifact_inventory_requires_native_comms_target(self):
        import json
        artifact = {"reason": "compiler-artifact", "executable": "comms-test", "profile": {"test": True}, "target": {"name": "comms"}}
        output = json.dumps(artifact) + "\n"
        self.assertEqual(guard.artifact_targets(output, {"comms"}), {"comms": "comms-test"})
        self.assertEqual(guard.artifact_targets(output, {"removed"}), {})
        with self.assertRaisesRegex(ValueError, "duplicate executable"):
            guard.artifact_targets(output + output, {"comms"})
        with self.assertRaisesRegex(ValueError, "missing required target"):
            guard.verify_targets({"comms": ["pulse::native::tests::native_frame_mapping_preserves_channels_and_explicit_throttle_bypass"]}, {})

    def test_native_external_vendor_case_stays_ignored_by_default(self):
        with mock.patch.object(guard, "run", side_effect=["required: test\nexternal: test\n", "external: test\n"]):
            guard.check_executable(["required"], "fake", "vendor", ["external"])
        with mock.patch.object(guard, "run", side_effect=["required: test\nexternal: test\n", ""]):
            with self.assertRaisesRegex(ValueError, "remain ignored by default"):
                guard.check_executable(["required"], "fake", "vendor", ["external"])

    def test_actual_runtime_script_rejects_removed_duplicate_and_filtered_bodies(self):
        script = (ROOT / "tooling/dev-test.sh").read_text()
        guard.validate_runtime_script(script)
        for bad in [script.replace(guard.VENDOR, guard.VENDOR + " -- --ignored"),
                    script.replace(guard.VENDOR, ""), script + "\n" + guard.VENDOR,
                    script.replace('catalyst-test ;;', 'catalyst-test NODE_CRATES=-p catalyst-comms ;;'),
                    script.replace('make -C "$FOUNDATION_DEV_ROOT" catalyst-test ;;', ''),
                    script + '\nmake -C "$FOUNDATION_DEV_ROOT" catalyst-test']:
            with self.subTest(bad=bad[-100:]), self.assertRaises(ValueError):
                guard.validate_runtime_script(bad)


    def test_vendor_examples_compile_gate_cannot_be_removed_filtered_or_conditional(self):
        script = (ROOT / "tooling/dev-test.sh").read_text()
        command = "cargo check --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --all-features --all-targets"
        for bad in [script.replace(command, ""), script + "\n" + command,
                    script.replace(command, command.replace("--all-features", "")),
                    script.replace(command, command + " || true"),
                    script.replace(command, "if false; then " + command + "; fi")]:
            with self.subTest(bad=bad[-100:]), self.assertRaisesRegex(ValueError, "vendor compile"):
                guard.validate_runtime_script(bad)

    def test_runtime_make_overrides_cannot_hide_in_continuations(self):
        script = (ROOT / "tooling/dev-test.sh").read_text()
        invocation = 'make -C "$FOUNDATION_DEV_ROOT" catalyst-test ;;'
        for variable in ["MAKEFLAGS", "MFLAGS", "MAKEOVERRIDES"]:
            for prefix in [f'{variable}="NODE_CRATES=-p catalyst-comms" ',
                           f'{variable}="NODE_CRATES=-p catalyst-comms" ' + "\\\n      "]:
                with self.subTest(variable=variable, prefix=prefix), self.assertRaises(ValueError):
                    guard.validate_runtime_script(script.replace(invocation, prefix + invocation))

    def test_bevy_runtime_is_in_the_enabled_expected_job_and_step(self):
        workflow = (ROOT / ".github/workflows/bevy-explorer-ci.yml").read_text()
        guard.validate_bevy_workflow(workflow)
        command = "          " + guard.BEVY + "\n"
        for bad in [workflow.replace("    name: Test Suite", "    if: false\n    name: Test Suite"),
                    workflow.replace("  test:\n", "  renamed-test:\n"),
                    workflow.replace(command, ""), workflow + command,
                    workflow.replace(command, "        if: false\n" + command),
                    workflow.replace(command, "          echo disabled\n")]:
            with self.subTest(bad=bad[-100:]), self.assertRaises(ValueError):
                guard.validate_bevy_workflow(bad)

    def test_shared_vendor_resolution_rejects_fork_and_registry_lock(self):
        guard.verify_resolution()
        with tempfile.TemporaryDirectory(prefix="enet-resolution-") as tmp:
            root = Path(tmp)
            for relative in ["services/catalyst/Cargo.toml", "services/catalyst/Cargo.lock",
                             "services/catalyst/comms/Cargo.toml", "rendering/bevy-explorer/source/Cargo.toml",
                             "rendering/bevy-explorer/source/Cargo.lock", "rendering/bevy-explorer/source/crates/comms/Cargo.toml"]:
                destination = root / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / relative, destination)
            guard.verify_resolution(root)
            manifest = root / "rendering/bevy-explorer/source/Cargo.toml"
            original = manifest.read_text()
            manifest.write_text(original.replace('../../../library/vendor/rusty_enet', '../../../library/vendor/client_fork'))
            with self.assertRaisesRegex(ValueError, "single pinned vendor path"):
                guard.verify_resolution(root)
            manifest.write_text(original)
            lock = root / "services/catalyst/Cargo.lock"
            lock.write_text(lock.read_text().replace('name = "rusty_enet"\nversion = "0.4.0"', 'name = "rusty_enet"\nversion = "0.4.0"\nsource = "registry+https://example.invalid/index"'))
            with self.assertRaisesRegex(ValueError, "exactly one local"):
                guard.verify_resolution(root)

    def test_missing_or_renamed_target(self):
        # The executable map, not source attribute scanning, defines available targets.
        with unittest.mock.patch.object(guard, "run", return_value='{"reason":"build-finished","success":true}\n'):
            targets = guard.compiled_targets([], ROOT)
            with self.assertRaisesRegex(ValueError, "missing required target"):
                guard.verify_targets({"removed": ["required"]}, targets)
        with self.assertRaisesRegex(ValueError, "missing required target"):
            guard.verify_targets({"renamed": ["required"]}, {"old": "unused"})


class DevScriptTests(unittest.TestCase):
    def run_fixture(self, scenario="good", ci="true"):
        bash = shutil.which("bash")
        if os.name == "nt":
            bash = str(Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "Git/bin/bash.exe")
        if not bash or not Path(bash).exists():
            self.skipTest("bash unavailable")
        with tempfile.TemporaryDirectory(prefix="dev-regression-") as tmp:
            root = Path(tmp)
            tools = root / "bin"
            tools.mkdir()
            log = root / "calls"
            def executable(name, body):
                path = tools / name
                path.write_text("#!/bin/bash\nset -eu\n" + body + "\n", newline="\n")
                path.chmod(0o755)
            executable("foundation-dev", r'''echo "foundation:$*" >> "$CALLS"
if [ "$1" = env ]; then
  printf 'export FOUNDATION_DEV_ROOT=%q\n' "$FIXTURE_ROOT"
  if [ "$SCENARIO" != unset-pg ]; then
    printf 'export FOUNDATION_DEV_TEST_DATABASE_URL=%q\n' "$FIXTURE_PG"
  fi
fi''')
            executable("nats-server", 'echo "nats:$*" >> "$CALLS"; [ "$SCENARIO" != bad-nats ]; echo "nats-server fixture"')
            executable("python3", 'echo "python:$*" >> "$CALLS"; [ "$SCENARIO" != guard-fail ]')
            executable("cargo", 'echo "cargo:$*" >> "$CALLS"; if [ "$1" = check ] && [ "$SCENARIO" = check-fail ]; then exit 7; fi')
            executable("make", r'''echo "make:$*" >> "$CALLS"
[ "$PULSE_NATS_SERVER_BIN" = "$ARCHIPELAGO_NATS_SERVER_BIN" ]
[ "$PULSE_NATS_SERVER_BIN" = "$(command -v nats-server)" ]
[ "$CATALYST_TEST_DATABASE_URL" = "$FIXTURE_PG" ]
[ "${GITHUB_ACTIONS-unset}" = "$EXPECTED_CI" ]
echo verified-exports >> "$CALLS"''')
            if scenario == "missing-nats":
                (tools / "nats-server").unlink()
            environment = dict(os.environ, CALLS=log.as_posix(), FIXTURE_ROOT=root.as_posix(),
                               FIXTURE_PG=" " if scenario == "blank-pg" else "postgres://fixture",
                               SCENARIO=scenario, EXPECTED_CI=ci if ci is not None else "unset")
            environment.pop("FOUNDATION_DEV_TEST_DATABASE_URL", None)
            if scenario == "missing-pg":
                environment["FIXTURE_PG"] = ""
            if ci is None:
                environment.pop("GITHUB_ACTIONS", None)
            else:
                environment["GITHUB_ACTIONS"] = ci
            # Git Bash converts Windows paths passed through PATH differently; set it in bash.
            command = 'export PATH="$1:/usr/bin:/bin"; bash "$2" rust'
            if os.name == "nt":
                command = 'export PATH="$(cygpath -u "$1"):/usr/bin:/bin"; bash "$2" rust'
            result = subprocess.run([bash, "-c", command, "fixture", tools.as_posix(),
                                     (ROOT / "tooling/dev-test.sh").as_posix()], env=environment,
                                    text=True, capture_output=True)
            return result, log.read_text() if log.exists() else ""

    def test_good_exports_cleanup_and_one_broad_run(self):
        for ci in ["true", "false", None]:
            result, calls = self.run_fixture(ci=ci)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(calls.count("make:"), 1, calls)
            self.assertIn("verified-exports", calls)
            self.assertEqual(calls.count("foundation:down"), 1, calls)
            self.assertIn("foundation:up --postgres-only", calls)
            self.assertIn("foundation:wait --postgres-only", calls)

    def test_vendor_compile_failure_stops_before_runtime(self):
        result, calls = self.run_fixture("check-fail")
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("cargo:check --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --all-features --all-targets", calls)
        self.assertNotIn("cargo:test", calls)
        self.assertNotIn("make:", calls)
        self.assertEqual(calls.count("foundation:down"), 1, calls)

    def test_missing_bad_prerequisites_and_guard_never_run_bodies(self):
        for scenario in ["missing-nats", "bad-nats", "missing-pg", "unset-pg", "blank-pg", "guard-fail"]:
            with self.subTest(scenario=scenario):
                result, calls = self.run_fixture(scenario)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                if scenario in ["missing-pg", "unset-pg", "blank-pg"]:
                    self.assertIn("FOUNDATION_DEV_TEST_DATABASE_URL must name disposable PostgreSQL", result.stderr)
                    self.assertNotIn("unbound variable", result.stderr)
                self.assertNotIn("make:", calls)
                self.assertNotIn("cargo:", calls)
                self.assertEqual(calls.count("foundation:down"), 1, calls)


if __name__ == "__main__":
    unittest.main()
