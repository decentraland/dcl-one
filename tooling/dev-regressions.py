#!/usr/bin/env python3
"""Fail closed on required libtest inventory; listing never executes test bodies."""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parent.parent
SELECTION = "--workspace --exclude dcl-scene-cli --exclude dcl-scene-authoring --exclude dcl-scene-build --exclude dcl-preview-tunnel --exclude dcl-preview-tunnel-protocol"
BROAD = "cd services/catalyst && cargo test --locked " + SELECTION
VENDOR = "cargo test --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --lib"
VENDOR_CHECK = "cargo check --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --all-features --all-targets"
BEVY = "cargo test --all --release --locked -- --test-threads=1"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def inventory(output):
    names = []
    for line in output.splitlines():
        if not line.strip():
            continue
        match = re.fullmatch(r"([^\s]+): (test|benchmark)", line)
        require(match is not None, f"malformed libtest inventory: {line!r}")
        if match[2] == "test":
            names.append(match[1])
    require(len(names) == len(set(names)), "duplicate libtest inventory")
    return Counter(names)


def verify(required, normal, ignored, label):
    require(required and len(required) == len(set(required)), f"{label}: empty/duplicate manifest")
    all_names, ignored_names = inventory(normal), inventory(ignored)
    for name in required:
        require(all_names[name] == 1, f"{label}: required test absent/renamed: {name}")
        require(ignored_names[name] == 0, f"{label}: required test ignored: {name}")
    print(f"{label}: {len(required)} mandatory tests present once, zero ignored", flush=True)


def validate_invocation(output, environ):
    # Deliberately a fixed contract, not a Makefile or shell parser. make -n sees
    # command-line/environment overrides exactly as the following real make does.
    require(output.strip() == BROAD, "catalyst-test must run the unfiltered locked workspace including catalyst; remove target/name/skip/ignored overrides")
    for variable in ("MAKEFLAGS", "MFLAGS", "MAKEOVERRIDES"):
        require(not environ.get(variable, "").strip(), f"unexpected {variable} override")


def validate_runtime_script(script):
    # Shell removes escaped newlines before recognizing assignments/commands.
    logical = re.sub(r"\\\r?\n", "", script)
    make_lines = [line.strip() for line in logical.splitlines() if re.search(r"\bmake\s", line)]
    require(make_lines == ['make -C "$FOUNDATION_DEV_ROOT" catalyst-test ;;'], "dev-test Rust runtime must invoke broad catalyst-test exactly once without filters")
    vendor_lines = [line.strip() for line in logical.splitlines() if line.strip().startswith("cargo test")]
    require(vendor_lines == [VENDOR], "vendor runtime must be the unfiltered locked default lib suite exactly once")

    compile_lines = [line.strip() for line in logical.splitlines() if line.strip().startswith("cargo check")]
    require(compile_lines == [VENDOR_CHECK], "vendor compile gate must check locked all-features/all-targets exactly once")
    sequence = "      python3 tooling/dev-regressions.py\n      " + VENDOR_CHECK + "\n      " + VENDOR + "\n"
    require(logical.count(sequence) == 1, "vendor compile gate must run unconditionally before the default runtime")


def validate_bevy_workflow(workflow):
    # Fixed indentation/shape contract for this checked-in workflow, not a YAML
    # evaluator. Fail closed if the job, selection condition or step changes.
    jobs = re.findall(r"^  test:\n(.*?)(?=^  [\w-]+:|\Z)", workflow, re.M | re.S)
    require(len(jobs) == 1, "native Bevy test job missing/duplicated")
    job = jobs[0]
    condition = "    if: ${{ needs.changes.outputs.test == 'true' && (needs.changes.outputs.native-matrix == 'true' || github.actor != 'dependabot[bot]') }}"
    require([line for line in job.splitlines() if line.startswith("    if:")] == [condition], "native Bevy test job selection condition changed/disabled")
    steps = re.findall(r"^      - name: Run the native test suite\n(.*?)(?=^      - |^    [\w-]+:|\Z)", job, re.M | re.S)
    require(len(steps) == 1, "native Bevy runtime step missing/duplicated")
    step = steps[0]
    require(not re.search(r"^        if:", step, re.M), "native Bevy runtime step must not be conditional")
    require("        run: |\n" in step, "native Bevy runtime step must run its script")
    command = "          " + BEVY + "\n"
    require(workflow.count(command) == 1 and step.endswith(command), "native Bevy runtime invocation moved/changed; retain the unfiltered lane")
    require("        working-directory: rendering/bevy-explorer/source\n" in job, "native Bevy workspace default changed")


def run(command, cwd=ROOT):
    result = subprocess.run(command, cwd=cwd, text=True, stdout=subprocess.PIPE, check=True)
    return result.stdout


def compiled_targets(arguments, cwd):
    output = run(["cargo", "test", "--locked", *arguments, "--no-run", "--message-format=json"], cwd)
    return artifact_targets(output)


def artifact_targets(output, selected=None):
    targets = {}
    for line in output.splitlines():
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact" and message.get("executable") and message.get("profile", {}).get("test"):
            name = message["target"]["name"]
            if selected is not None and name not in selected:
                continue
            require(name not in targets, f"duplicate executable target: {name}")
            targets[name] = message["executable"]
    return targets


def verify_resolution(root=ROOT):
    vendor = (root / "library/vendor/rusty_enet").resolve()
    for workspace, relative in [("services/catalyst", "../../library/vendor/rusty_enet"),
                                ("rendering/bevy-explorer/source", "../../../library/vendor/rusty_enet")]:
        directory = root / workspace
        manifest = tomllib.loads((directory / "Cargo.toml").read_text())
        dependency = manifest["workspace"]["dependencies"]["rusty_enet"]
        require(dependency == {"path": relative, "version": "0.4.0"}, f"{workspace}: ENet must use the single pinned vendor path")
        require((directory / dependency["path"]).resolve() == vendor, f"{workspace}: ENet vendor path diverged")
        lock = tomllib.loads((directory / "Cargo.lock").read_text())
        entries = [entry for entry in lock["package"] if entry["name"] == "rusty_enet"]
        require(len(entries) == 1 and entries[0]["version"] == "0.4.0" and "source" not in entries[0] and "checksum" not in entries[0], f"{workspace}: ENet lock must resolve exactly one local 0.4.0")
    for consumer in ["services/catalyst/comms", "rendering/bevy-explorer/source/crates/comms"]:
        manifest = tomllib.loads((root / consumer / "Cargo.toml").read_text())
        dependencies = manifest["dependencies"] if consumer.startswith("services/") else manifest["target"]['cfg(not(target_arch = "wasm32"))']["dependencies"]
        require(dependencies.get("rusty_enet") == {"workspace": True}, f"{consumer}: ENet must inherit the shared workspace dependency")


def verify_targets(required, targets):
    for target, names in required.items():
        require(target in targets, f"missing required target: {target}")
        check_executable(names, targets[target], target)


def check_executable(required, executable, label, required_ignored=()):
    normal = run([executable, "--list", "--format", "terse"])
    ignored = run([executable, "--list", "--ignored", "--format", "terse"])
    verify(required, normal, ignored, label)
    for name in required_ignored:
        require(inventory(normal)[name] == 1 and inventory(ignored)[name] == 1, f"{label}: provisioned external test must remain ignored by default: {name}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bevy-artifacts", type=Path)
    args = parser.parse_args()
    manifest = json.loads((ROOT / "tooling/dev-regressions.json").read_text())
    verify_resolution()
    if args.bevy_artifacts:
        workflow = (ROOT / ".github/workflows/bevy-explorer-ci.yml").read_text()
        validate_bevy_workflow(workflow)
        targets = artifact_targets(args.bevy_artifacts.read_text(encoding="utf-8-sig"), {"comms"})
        require("comms" in targets, "native Bevy comms lib target missing")
        verify_targets(manifest["bevy"], targets)
        return
    validate_runtime_script((ROOT / "tooling/dev-test.sh").read_text())
    validate_invocation(run(["make", "--no-print-directory", "-n", "catalyst-test"]), os.environ)
    required = manifest["catalyst"]
    if os.name != "nt":
        for target, names in manifest["unix"].items():
            required[target] += names
    selection = SELECTION.split()
    for target in required:
        selection += ["--test", target]
    targets = compiled_targets(selection, ROOT / "services/catalyst")
    verify_targets(required, targets)
    vendor_targets = compiled_targets(["--manifest-path", "library/vendor/rusty_enet/Cargo.toml", "--lib"], ROOT)
    require("rusty_enet" in vendor_targets, "missing vendor lib target")
    check_executable(manifest["vendor"], vendor_targets["rusty_enet"], "vendored ENet deterministic suite", manifest["vendor_ignored"])


if __name__ == "__main__":
    try:
        main()
    except (ValueError, subprocess.CalledProcessError) as error:
        sys.exit(f"mandatory regression guard: {error}")
