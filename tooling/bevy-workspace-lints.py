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


def path_dependencies(manifest, document):
    """The directories of the `path` dependencies a manifest declares, in any dependency table."""
    tables = [document, document.get("workspace", {}), *document.get("target", {}).values()]
    for table in tables:
        for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
            for dependency in table.get(kind, {}).values():
                if isinstance(dependency, dict) and "path" in dependency:
                    yield (manifest.parent / dependency["path"]).resolve()


def member_manifests():
    """The root manifest, the `members` globs and, as Cargo does, every path dependency inside the
    workspace directory, transitively; `exclude` removes a directory and everything below it."""
    root = SOURCE / "Cargo.toml"
    workspace = tomllib.loads(root.read_text())["workspace"]
    excluded = [(SOURCE / path).resolve() for path in workspace.get("exclude", [])]
    pending = [root.resolve()] + [(directory / "Cargo.toml").resolve()
                                  for pattern in workspace["members"] for directory in SOURCE.glob(pattern)]
    found = {}
    while pending:
        manifest = pending.pop()
        if manifest in found or not manifest.is_file():
            continue
        if not manifest.is_relative_to(SOURCE.resolve()) or any(manifest.is_relative_to(path) for path in excluded):
            continue
        found[manifest] = document = tomllib.loads(manifest.read_text())
        pending.extend(directory / "Cargo.toml" for directory in path_dependencies(manifest, document))
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
