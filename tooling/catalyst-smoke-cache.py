#!/usr/bin/env python3
"""Drop superseded Rust libraries before saving the CI smoke build snapshot."""
import json
from pathlib import Path
import re
import subprocess


def prune(deps, messages):
    # An rlib and its companion rmeta share a stem. Keep both for every active variant.
    library = re.compile(r"(lib.+)-[0-9a-f]{16}\.(?:rlib|rmeta)")
    live = set()
    names = set()
    for line in messages.splitlines():
        artifact = json.loads(line)
        if artifact["reason"] != "compiler-artifact":
            continue
        for filename in artifact["filenames"]:
            path = Path(filename)
            match = library.fullmatch(path.name)
            if path.parent == deps and match:
                live.add(path.stem)
                names.add(match[1])
    if not live:
        raise ValueError("Cargo reported no Rust libraries in the smoke target")
    removed = 0
    for path in deps.iterdir():
        match = library.fullmatch(path.name)
        if match and match[1] in names and path.stem not in live:
            removed += path.stat().st_size
            path.unlink()
    return removed


if __name__ == "__main__":
    workspace = Path(__file__).resolve().parents[1] / "services/catalyst"
    target = workspace / "target"
    result = subprocess.check_output(
        ["cargo", "build", "--locked", "--bin", "catalyst", "--target-dir", str(target),
         "--message-format=json-render-diagnostics"], cwd=workspace, text=True)
    removed = prune(target / "debug/deps", result)
    print(f"Removed {removed:,} bytes of superseded Rust library files from the smoke snapshot.")
