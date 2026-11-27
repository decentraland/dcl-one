#!/usr/bin/env python3
"""Drop superseded Rust build artifacts before saving the CI smoke build snapshot."""
import filecmp
import json
import os
from pathlib import Path
import re
import subprocess


def prune(deps, messages):
    # Rust library metadata and its rlib share a stem even when Cargo lists only one.
    library = re.compile(r"(lib.+)-[0-9a-f]{16}\.(rlib|rmeta|so|dylib)")
    executable = re.compile(r"catalyst-[0-9a-f]{16}")
    app = deps.parent / "catalyst"
    reported = set()
    for line in messages.splitlines():
        artifact = json.loads(line)
        if artifact["reason"] == "compiler-artifact":
            reported.update(Path(name) for name in artifact["filenames"])

    def family(match):
        suffix = match[2]
        return match[1], "rlib" if suffix == "rmeta" else suffix

    active = {}
    for path in reported:
        match = library.fullmatch(path.name)
        if path.parent == deps and match:
            active.setdefault(family(match), set()).add(path.stem)
    if not active:
        raise ValueError("Cargo reported no Rust libraries in the smoke target")

    # Cargo publishes debug/catalyst as a hard link or a copy of the hashed executable.
    obsolete = set()
    binaries = {p for p in deps.glob("catalyst-*") if executable.fullmatch(p.name)
                and not p.is_symlink() and p.is_file()}
    retained = set()
    if app in reported and app.is_file():
        retained = {p for p in binaries if p.samefile(app)}
        if not retained:
            retained = {p for p in binaries if filecmp.cmp(app, p, shallow=False)}
    if retained:
        obsolete.update(binaries - retained - reported)
    elif binaries:
        print(f"Kept all {len(binaries)} hashed Catalyst binaries because the current executable "
              "could not be verified against Cargo's reported files.")
    for path in deps.iterdir():
        match = library.fullmatch(path.name)
        if match and (stems := active.get(family(match))) and path.stem not in stems:
            obsolete.add(path)
    removed = 0
    for path in obsolete:
        info = path.lstat()
        path.unlink()
        if info.st_nlink == 1:
            removed += info.st_size
    return removed


def build_env(environ):
    """The build environment, linking the nix shell's V8 (flake.nix) when it is there."""
    env = dict(environ)
    if env.get("CATALYST_RUSTY_V8_ARCHIVE"):
        env["RUSTY_V8_ARCHIVE"] = env["CATALYST_RUSTY_V8_ARCHIVE"]
    return env


if __name__ == "__main__":
    workspace = Path(__file__).resolve().parents[1] / "services/catalyst"
    target = workspace / "target"
    result = subprocess.check_output(
        ["cargo", "build", "--locked", "--bin", "catalyst", "--target-dir", str(target),
         "--message-format=json-render-diagnostics"], cwd=workspace, env=build_env(os.environ),
        text=True)
    removed = prune(target / "debug/deps", result)
    print(f"Removed {removed:,} bytes of superseded Rust build artifacts from the smoke snapshot.")
