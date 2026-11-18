#!/usr/bin/env python3
"""Fail when a package of the Explorer workspace does not inherit the workspace lints.

`[workspace.lints]` in rendering/bevy-explorer/source/Cargo.toml reaches only the packages
whose manifest holds exactly `[lints] workspace = true`. Every workspace member must, so a lint
added there applies to the whole workspace. Offline: reads the manifests, runs no cargo.
"""

import glob
import sys
import tomllib
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "rendering/bevy-explorer/source"


def path_dependencies(manifest, document, inherited):
    """The directories of the `path` dependencies a package declares in any dependency table, with
    a `workspace = true` entry resolved through the root's `[workspace.dependencies]`."""
    for table in [document, *document.get("target", {}).values()]:
        for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
            for name, dependency in table.get(kind, {}).items():
                if not isinstance(dependency, dict):
                    continue
                if dependency.get("workspace") is True:
                    dependency = inherited.get(name, {})
                    if isinstance(dependency, dict) and "path" in dependency:
                        yield (SOURCE / dependency["path"]).resolve()
                elif "path" in dependency:
                    yield (manifest.parent / dependency["path"]).resolve()


def member_manifests():
    """As Cargo resolves them: the root package, the `members` entries and every path dependency
    inside the workspace directory, transitively. `exclude` removes a directory and everything
    below it, except a member listed by name rather than by glob."""
    root = (SOURCE / "Cargo.toml").resolve()
    workspace = tomllib.loads(root.read_text())["workspace"]
    inherited = workspace.get("dependencies", {})
    excluded = [(SOURCE / path).resolve() for path in workspace.get("exclude", [])]
    named = {(SOURCE / pattern / "Cargo.toml").resolve() for pattern in workspace["members"] if not glob.has_magic(pattern)}
    pending = [root, *named] + [(directory / "Cargo.toml").resolve()
                                for pattern in workspace["members"] for directory in SOURCE.glob(pattern)]
    found = {}
    while pending:
        manifest = pending.pop()
        if manifest in found or not manifest.is_file() or not manifest.is_relative_to(SOURCE.resolve()):
            continue
        if manifest not in named and any(manifest.is_relative_to(path) for path in excluded):
            continue
        found[manifest] = document = tomllib.loads(manifest.read_text())
        pending.extend(directory / "Cargo.toml" for directory in path_dependencies(manifest, document, inherited))
    return {manifest: document for manifest, document in sorted(found.items()) if "package" in document}


def main():
    missing = [
        manifest.relative_to(SOURCE.resolve())
        for manifest, document in member_manifests().items()
        if document.get("lints") != {"workspace": True}
    ]
    for manifest in missing:
        print(f"::error file=rendering/bevy-explorer/source/{manifest}::"
              "add `[lints] workspace = true` so the workspace lints reach this package")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
