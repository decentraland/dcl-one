#!/usr/bin/env python3
"""Fail when a package of the Explorer workspace does not inherit the workspace lints.

`[workspace.lints]` in rendering/bevy-explorer/source/Cargo.toml reaches only the packages
whose manifest holds exactly `[lints] workspace = true`. Every workspace member must, so a lint
added there applies to the whole workspace. Offline: reads the manifests, runs no cargo.
"""

import glob
import os
import sys
import tomllib
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parents[1]
SOURCE = REPOSITORY / "rendering/bevy-explorer/source"


def normal(path):
    """An absolute path with `.` and `..` removed and symlinks kept, as Cargo compares them."""
    return Path(os.path.normpath(path))


def path_dependencies(manifest, document, inherited):
    """The manifests of the `path` dependencies a package declares in any dependency table, with
    a `workspace = true` entry resolved through the root's `[workspace.dependencies]`."""
    for table in [document, *document.get("target", {}).values()]:
        for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
            for name, dependency in table.get(kind, {}).items():
                if not isinstance(dependency, dict):
                    continue
                if dependency.get("workspace") is True:
                    dependency = inherited.get(name, {})
                    if isinstance(dependency, dict) and "path" in dependency:
                        yield normal(SOURCE / dependency["path"] / "Cargo.toml")
                elif "path" in dependency:
                    yield normal(manifest.parent / dependency["path"] / "Cargo.toml")


def member_manifests():
    """As Cargo resolves them: the root package, the `members` entries and every path dependency,
    transitively. A path dependency outside the workspace directory is a member only when its
    `package.workspace` names this root. `exclude` removes a directory and everything below it,
    except what lies under a member listed by name rather than by glob."""
    root = SOURCE / "Cargo.toml"
    workspace = tomllib.loads(root.read_text())["workspace"]
    inherited = workspace.get("dependencies", {})
    excluded = [normal(SOURCE / path) for path in workspace.get("exclude", [])]
    named = [normal(SOURCE / pattern) for pattern in workspace["members"] if not glob.has_magic(pattern)]
    listed = [normal(SOURCE / pattern / "Cargo.toml") for pattern in workspace["members"] if not glob.has_magic(pattern)]
    listed += [normal(directory / "Cargo.toml") for pattern in workspace["members"] if glob.has_magic(pattern)
               for directory in SOURCE.glob(pattern)]
    pending = [(root, False)] + [(manifest, False) for manifest in listed]
    found = {}
    while pending:
        manifest, is_path_dependency = pending.pop()
        if manifest in found or not manifest.is_file():
            continue
        document = tomllib.loads(manifest.read_text())
        if is_path_dependency and not manifest.parent.is_relative_to(SOURCE):
            pointer = document.get("package", {}).get("workspace")
            if pointer is None or normal(manifest.parent / pointer) != SOURCE:
                continue
        if (not any(manifest.is_relative_to(member) for member in named)
                and any(manifest.is_relative_to(path) for path in excluded)):
            continue
        found[manifest] = document
        pending.extend((dependency, True) for dependency in path_dependencies(manifest, document, inherited))
    return {manifest: document for manifest, document in sorted(found.items()) if "package" in document}


def main():
    missing = [
        os.path.relpath(manifest, REPOSITORY)
        for manifest, document in member_manifests().items()
        if document.get("lints") != {"workspace": True}
    ]
    for manifest in missing:
        print(f"::error file={manifest}::"
              "add `[lints] workspace = true` so the workspace lints reach this package")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
