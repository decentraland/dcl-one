#!/usr/bin/env python3
"""Fail when a package of the Explorer workspace does not inherit the workspace lints.

`[workspace.lints]` in rendering/bevy-explorer/source/Cargo.toml reaches only the packages
whose manifest holds exactly `[lints] workspace = true`. Every workspace member must, so a lint
added there applies to the whole workspace. Offline: reads the manifests, runs no cargo.
"""

import sys
import tomllib
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "rendering/bevy-explorer/source"


def member_manifests():
    workspace = tomllib.loads((SOURCE / "Cargo.toml").read_text())["workspace"]
    excluded = {(SOURCE / path).resolve() for path in workspace.get("exclude", [])}
    yield SOURCE / "Cargo.toml"
    for pattern in workspace["members"]:
        for directory in sorted(SOURCE.glob(pattern)):
            manifest = directory / "Cargo.toml"
            if directory.resolve() not in excluded and manifest.is_file():
                yield manifest


def main():
    missing = [
        manifest.relative_to(SOURCE)
        for manifest in member_manifests()
        if tomllib.loads(manifest.read_text()).get("lints") != {"workspace": True}
    ]
    for manifest in missing:
        print(f"::error file=rendering/bevy-explorer/source/{manifest}::"
              "add `[lints] workspace = true` so the workspace lints reach this package")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
